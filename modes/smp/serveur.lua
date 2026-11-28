-- SMP (Survival Multiplayer) : la survie a plusieurs, dans un monde garde d'une partie a l'autre. Le jeu
-- reste celui de la survie ; le mode y ajoute ce qu'on trouve sur les serveurs SMP : un point de depart
-- protege, des maisons ou revenir (/sethome, /home), la teleportation entre joueurs (/tpa), le retour
-- sur les lieux de sa mort (/back), le choix de se battre ou non (/pvp), le vote pour passer la nuit
-- (/dormir), de l'argent (/argent, /payer, /banque) et un hotel des ventes (/ah).

local SPAWN_RADIUS = 16     -- blocs proteges autour du point de depart
local WARMUP = 4            -- secondes sans bouger avant une teleportation
local HOMES = 3             -- maisons par joueur
local TPA_TIME = 60         -- secondes pendant lesquelles une demande de teleportation tient
local START_MONEY = 100     -- kobans offerts a l'arrivee
local AH_PER_PLAYER = 6     -- ventes en cours par joueur
local AH_MAX = 120          -- ventes en cours en tout
local AH_PAGE = 8           -- ventes par page dans la fenetre

rules.set("pvp", true)

-- ------------------------------------------------------------------------------------------------
-- Ce que le mode retient, enregistre avec le monde.
local function split(text, sep)
  local out = {}
  for piece in string.gmatch(text or "", "([^" .. sep .. "]+)") do out[#out + 1] = piece end
  return out
end

local spawn                 -- { x, y, z }
do
  local s = split(store.get("spawn"), ",")
  if #s == 3 then spawn = { tonumber(s[1]), tonumber(s[2]), tonumber(s[3]) } end
end

local function named(name)
  for _, p in ipairs(players()) do if p.name:lower() == (name or ""):lower() then return p end end
end

-- Argent : un compte en kobans par joueur, qui existe meme quand il n'est pas la.
local function money(name) return tonumber(store.get("m:" .. name) or "") or 0 end
local function set_money(name, v) store.set("m:" .. name, tostring(math.max(0, math.floor(v)))) end

-- Maisons : « nom=x,y,z,dimension » separees par des points-virgules.
local function homes(name)
  local out = {}
  for _, h in ipairs(split(store.get("h:" .. name), ";")) do
    local label, rest = h:match("^([^=]+)=(.+)$")
    local c = split(rest, ",")
    if label and #c == 4 then out[label] = { tonumber(c[1]), tonumber(c[2]), tonumber(c[3]), tonumber(c[4]) } end
  end
  return out
end
local function save_homes(name, list)
  local parts = {}
  for label, h in pairs(list) do parts[#parts + 1] = label .. "=" .. h[1] .. "," .. h[2] .. "," .. h[3] .. "," .. h[4] end
  table.sort(parts)
  store.set("h:" .. name, table.concat(parts, ";"))
end

-- ------------------------------------------------------------------------------------------------
-- Teleportations : on ne part qu'apres quelques secondes sans bouger.
local going = {}            -- pseudo -> { secondes, x, z, destination { x, y, z }, texte }
local died = {}             -- pseudo -> { x, y, z, dimension } : ou il est mort
local asks = {}             -- pseudo du destinataire -> { de = pseudo, reste = secondes }
local no_pvp = {}           -- pseudo -> il a renonce au combat
local sleepy = {}           -- pseudo -> il a vote pour passer la nuit

local function travel(p, to, text)
  local x, _, z = p:pos()
  going[p.name] = { WARMUP, x, z, to, text }
  return text .. " dans " .. WARMUP .. " secondes : ne bougez pas."
end

local function near_spawn(x, z)
  return spawn ~= nil and math.abs(x - spawn[1]) <= SPAWN_RADIUS and math.abs(z - spawn[3]) <= SPAWN_RADIUS
end

command("spawn", "Retourne au point de depart du monde", function(p)
  if not spawn then return "Le point de depart n'est pas encore connu." end
  if p:dimension() ~= 0 then return "On ne rejoint le point de depart que depuis la surface." end
  return travel(p, spawn, "Retour au point de depart")
end)

command("sethome", "Fixe une maison ici : /sethome [nom]", function(p, args)
  local label = (args ~= "" and args or "maison"):lower()
  if not label:match("^[%w_]+$") or #label > 16 then return "Un nom de maison : lettres, chiffres, 16 au plus." end
  local list = homes(p.name)
  local n = 0
  for _ in pairs(list) do n = n + 1 end
  if not list[label] and n >= HOMES then return "Vous avez deja " .. HOMES .. " maisons : /delhome <nom> en retire une." end
  local x, y, z = p:pos()
  list[label] = { math.floor(x) + 0.5, math.floor(y * 10) / 10, math.floor(z) + 0.5, p:dimension() }
  save_homes(p.name, list)
  return "Maison « " .. label .. " » fixee ici."
end)

command("home", "Retourne a une maison : /home [nom]", function(p, args)
  local label = (args ~= "" and args or "maison"):lower()
  local h = homes(p.name)[label]
  if not h then return "Vous n'avez pas de maison « " .. label .. " ». /homes les liste, /sethome en fixe une." end
  if h[4] ~= p:dimension() then return "Cette maison est dans un autre monde : rejoignez-le d'abord." end
  return travel(p, h, "Retour a la maison « " .. label .. " »")
end)

command("delhome", "Retire une maison : /delhome <nom>", function(p, args)
  local list = homes(p.name)
  local label = args:lower()
  if not list[label] then return "Vous n'avez pas de maison « " .. label .. " »." end
  list[label] = nil
  save_homes(p.name, list)
  return "Maison « " .. label .. " » retiree."
end)

command("homes", "Liste ses maisons", function(p)
  local names = {}
  for label in pairs(homes(p.name)) do names[#names + 1] = label end
  table.sort(names)
  return #names > 0 and ("Vos maisons : " .. table.concat(names, ", ")) or "Vous n'avez pas encore de maison : /sethome en fixe une."
end)

command("back", "Retourne la ou l'on est mort", function(p)
  local d = died[p.name]
  if not d then return "Vous n'etes pas mort depuis votre arrivee." end
  if d[4] ~= p:dimension() then return "Vous etes mort dans un autre monde." end
  died[p.name] = nil
  return travel(p, d, "Retour sur les lieux de votre mort")
end)

command("tpa", "Demande a rejoindre un joueur : /tpa <joueur>", function(p, args)
  local other = named(args)
  if not other or other.name == p.name then return "Ce joueur n'est pas en ligne." end
  if other:dimension() ~= p:dimension() then return other.name .. " est dans un autre monde." end
  asks[other.name] = { from = p.name, left = TPA_TIME }
  other:tell(p.name .. " demande a vous rejoindre : /tpaccept ou /tpdeny.")
  return "Demande envoyee a " .. other.name .. "."
end)

command("tpaccept", "Accepte la demande de teleportation recue", function(p)
  local ask = asks[p.name]
  local who = ask and named(ask.from)
  if not who then return "Aucune demande en attente." end
  asks[p.name] = nil
  local x, y, z = p:pos()
  who:tell(travel(who, { x, y, z }, p.name .. " accepte : depart"))
  return "Vous acceptez " .. who.name .. "."
end)

command("tpdeny", "Refuse la demande de teleportation recue", function(p)
  local ask = asks[p.name]
  if not ask then return "Aucune demande en attente." end
  asks[p.name] = nil
  local who = named(ask.from)
  if who then who:tell(p.name .. " refuse votre demande.") end
  return "Demande refusee."
end)

command("pvp", "Accepte ou refuse le combat entre joueurs : /pvp oui ou /pvp non", function(p, args)
  if args == "non" then no_pvp[p.name] = true elseif args == "oui" then no_pvp[p.name] = nil end
  store.set("pvp:" .. p.name, no_pvp[p.name] and "non" or "")
  return no_pvp[p.name] and "Vous ne vous battez pas : personne ne peut vous frapper, et vous ne frappez personne." or "Vous acceptez le combat."
end)

command("dormir", "Vote pour passer la nuit (il faut la moitie des joueurs)", function(p)
  local t = world.time()
  if t > 6.5 and t < 19 then return "Il ne fait pas nuit." end
  sleepy[p.name] = true
  local votes, total = 0, #players()
  for _, o in ipairs(players()) do if sleepy[o.name] then votes = votes + 1 end end
  if votes * 2 >= total then
    sleepy = {}
    world.set_time(7)
    say("La nuit passe : " .. votes .. " joueur" .. (votes > 1 and "s" or "") .. " sur " .. total .. " ont vote.")
    return "Bonjour !"
  end
  say(p.name .. " veut passer la nuit (" .. votes .. " / " .. math.ceil(total / 2) .. ") : /dormir pour voter.")
end)

command("regles", "Rappelle ce que le mode ajoute a la survie", function(p)
  p:tell("/spawn, /sethome [nom], /home [nom], /homes, /delhome <nom>, /back")
  p:tell("/tpa <joueur>, /tpaccept, /tpdeny - /pvp oui|non - /dormir")
  p:tell("/argent, /payer <joueur> <somme>, /banque deposer|retirer <kobans>")
  return "/ah ouvre l'hotel des ventes ; /ah vendre <prix> met en vente la pile que vous tenez."
end)

-- ------------------------------------------------------------------------------------------------
-- Argent.
command("argent", "Dit combien de kobans on a en banque", function(p) return "Vous avez " .. money(p.name) .. " kobans en banque." end)

command("payer", "Donne de l'argent a un joueur : /payer <joueur> <somme>", function(p, args)
  local w = split(args, " ")
  local other, sum = named(w[1]), math.floor(tonumber(w[2]) or 0)
  if not other or other.name == p.name then return "Ce joueur n'est pas en ligne." end
  if sum < 1 or sum > money(p.name) then return "Somme invalide : vous avez " .. money(p.name) .. " kobans." end
  set_money(p.name, money(p.name) - sum)
  set_money(other.name, money(other.name) + sum)
  other:tell(p.name .. " vous donne " .. sum .. " kobans.")
  return "Vous donnez " .. sum .. " kobans a " .. other.name .. "."
end)

-- La banque change les pieces (l'objet koban) en argent de compte, et l'inverse.
command("banque", "Change des kobans en argent : /banque deposer <n> ou /banque retirer <n>", function(p, args)
  local w = split(args, " ")
  local n = math.floor(tonumber(w[2]) or 0)
  if w[1] == "deposer" then
    if n < 1 then n = p:count("koban") or 0 end
    if n < 1 or not p:take("koban", n) then return "Vous n'avez pas ces kobans sur vous." end
    set_money(p.name, money(p.name) + n)
    return n .. " kobans deposes : " .. money(p.name) .. " en banque."
  elseif w[1] == "retirer" then
    if n < 1 or n > money(p.name) or n > 640 then return "Somme invalide : vous avez " .. money(p.name) .. " kobans en banque." end
    set_money(p.name, money(p.name) - n)
    p:give("koban", n)
    return n .. " kobans retires : " .. money(p.name) .. " en banque."
  end
  return "/banque deposer <n> ou /banque retirer <n>. Vous avez " .. money(p.name) .. " kobans en banque."
end)

-- ------------------------------------------------------------------------------------------------
-- L'hotel des ventes : chacun y met des piles en vente, tout le monde peut les acheter.
local sales = {}            -- { id, seller, price, token, item, n }
local next_id = tonumber(store.get("ah:next") or "") or 1

local function save_sales()
  local parts = {}
  for _, s in ipairs(sales) do parts[#parts + 1] = table.concat({ s.id, s.seller, s.price, s.token }, "|") end
  store.set("ah", table.concat(parts, ";"))
  store.set("ah:next", tostring(next_id))
end

for _, line in ipairs(split(store.get("ah"), ";")) do
  local f = split(line, "|")
  local t = split(f[4] or "", "*")
  if #f == 4 and #t == 4 then sales[#sales + 1] = { id = tonumber(f[1]), seller = f[2], price = tonumber(f[3]), token = f[4], item = t[1], n = tonumber(t[2]) } end
end

-- Envoie au script d'ecran la page demandee : le solde, puis une vente par ligne.
local function show(p, page)
  local pages = math.max(1, math.ceil(#sales / AH_PAGE))
  page = math.max(1, math.min(pages, page or 1))
  local lines = { page .. "/" .. pages .. "/" .. money(p.name) .. "/" .. #sales }
  for k = (page - 1) * AH_PAGE + 1, math.min(#sales, page * AH_PAGE) do
    local s = sales[#sales + 1 - k]     -- les plus recentes d'abord
    lines[#lines + 1] = table.concat({ s.id, s.item, s.n, s.price, s.seller, s.seller == p.name and "1" or "0" }, "|")
  end
  p:send("ah", table.concat(lines, "\n"))
end

local function find(id)
  for k, s in ipairs(sales) do if s.id == id then return k, s end end
end

local function buy(p, id)
  local k, s = find(id)
  if not s then return "Cette vente n'existe plus." end
  if s.seller == p.name then
    -- Sa propre vente : on la retire, l'objet revient.
    table.remove(sales, k)
    save_sales()
    p:give_stack(s.token)
    return "Vente retiree : l'objet vous est rendu."
  end
  if money(p.name) < s.price then return "Il vous manque " .. (s.price - money(p.name)) .. " kobans." end
  table.remove(sales, k)
  save_sales()
  set_money(p.name, money(p.name) - s.price)
  set_money(s.seller, money(s.seller) + s.price)
  p:give_stack(s.token)
  local seller = named(s.seller)
  if seller then seller:tell(p.name .. " achete votre " .. s.item .. " : +" .. s.price .. " kobans.") end
  return "Achete : " .. s.n .. " x " .. s.item .. " pour " .. s.price .. " kobans."
end

command("ah", "Hotel des ventes : /ah l'ouvre, /ah vendre <prix> met en vente la pile tenue", function(p, args)
  local w = split(args, " ")
  if w[1] == "vendre" then
    local price = math.floor(tonumber(w[2]) or 0)
    if price < 1 or price > 1000000 then return "/ah vendre <prix> : un prix de 1 a 1 000 000 kobans." end
    local mine = 0
    for _, s in ipairs(sales) do if s.seller == p.name then mine = mine + 1 end end
    if mine >= AH_PER_PLAYER then return "Vous avez deja " .. AH_PER_PLAYER .. " ventes en cours." end
    if #sales >= AH_MAX then return "L'hotel des ventes est plein." end
    local item, n, token = p:take_held()
    if not item then return "Tenez en main ce que vous voulez vendre." end
    sales[#sales + 1] = { id = next_id, seller = p.name, price = price, token = token, item = item, n = n }
    next_id = next_id + 1
    save_sales()
    say(p.name .. " met en vente " .. n .. " x " .. item .. " pour " .. price .. " kobans (/ah).")
    return "Mis en vente."
  elseif w[1] == "acheter" then
    return buy(p, tonumber(w[2]) or 0)
  end
  show(p, 1)
end)

on("message", function(p, name, data)
  if name == "ah_page" then
    show(p, tonumber(data) or 1)
  elseif name == "ah_acheter" then
    local id, page = data:match("^(%d+)/(%d+)$")
    p:send("ah_note", buy(p, tonumber(id) or 0) or "")
    show(p, tonumber(page) or 1)
  end
end)

-- ------------------------------------------------------------------------------------------------
-- La vie du monde.
on("join", function(p)
  p:set_mode("survie")
  if not spawn then
    local x, y, z = p:pos()
    if x then
      spawn = { math.floor(x) + 0.5, y, math.floor(z) + 0.5 }
      store.set("spawn", spawn[1] .. "," .. spawn[2] .. "," .. spawn[3])
    end
  end
  no_pvp[p.name] = store.get("pvp:" .. p.name) == "non" or nil
  -- La premiere fois : de quoi commencer, et un peu d'argent.
  if store.get("vu:" .. p.name) ~= "1" then
    store.set("vu:" .. p.name, "1")
    set_money(p.name, START_MONEY)
    p:give("pioche_en_pierre", 1)
    p:give("hache_ono_en_pierre", 1)
    p:give("onigiri", 8)
    say("Bienvenue a " .. p.name .. ", qui rejoint le SMP pour la premiere fois !")
  end
  p:tell("SMP : /regles liste les commandes. Vous avez " .. money(p.name) .. " kobans en banque.")
end)

on("leave", function(p)
  going[p.name], asks[p.name], sleepy[p.name] = nil, nil, nil
end)

on("death", function(p)
  local x, y, z = p:pos()
  if x then
    died[p.name] = { x, y, z, p:dimension() }
    p:tell("Vous etes mort en " .. math.floor(x) .. ", " .. math.floor(y) .. ", " .. math.floor(z) .. " : /back vous y ramene.")
  end
end)

-- Le point de depart est protege : on n'y casse rien, on n'y pose rien, on ne s'y bat pas.
local function guarded(p, x, z)
  if near_spawn(x, z) then
    p:info("Le point de depart est protege.")
    return true
  end
end
on("break", function(p, x, y, z) if guarded(p, x, z) then return false end end)
on("place", function(p, x, y, z) if guarded(p, x, z) then return false end end)

on("hit", function(p, victim)
  if no_pvp[p.name] or no_pvp[victim] then
    p:info(no_pvp[p.name] and "Vous avez renonce au combat (/pvp oui)." or (victim .. " ne se bat pas."))
    return false
  end
  local x, _, z = p:pos()
  if x and near_spawn(x, z) then
    p:info("On ne se bat pas au point de depart.")
    return false
  end
end)

local ticks = 0
on("tick", function()
  ticks = ticks + 1
  if ticks % 20 ~= 0 then return end
  for name, ask in pairs(asks) do
    ask.left = ask.left - 1
    if ask.left <= 0 then asks[name] = nil end
  end
  -- Le jour revenu, les votes de la nuit sont oublies.
  local t = world.time()
  if t > 7 and t < 19 then sleepy = {} end
  for _, p in ipairs(players()) do
    local go = going[p.name]
    if go then
      local x, _, z = p:pos()
      if x and (math.abs(x - go[2]) > 0.5 or math.abs(z - go[3]) > 0.5) then
        going[p.name] = nil
        p:info("Vous avez bouge : teleportation annulee.")
      else
        go[1] = go[1] - 1
        if go[1] <= 0 then
          going[p.name] = nil
          p:teleport(go[4][1], go[4][2] + 0.2, go[4][3])
        end
      end
    end
  end
end)
