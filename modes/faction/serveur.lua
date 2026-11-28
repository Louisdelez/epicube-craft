-- PvP Faction : un monde de survie garde d'une partie a l'autre, ou les joueurs se regroupent en
-- factions. Une faction revendique des terres, chunk par chunk : chez elle, elle seule batit, casse et
-- ouvre les coffres. Ce qu'elle peut tenir depend de sa puissance (son « power ») : chaque membre en
-- apporte, en perd a chaque mort, en regagne avec le temps. Une faction qui tient plus de terres qu'elle
-- n'a de puissance peut se les faire prendre. Les factions se declarent alliees ou ennemies ; les
-- explosifs, eux, ne respectent aucune frontiere : c'est ainsi qu'on pille une base.
-- Tout passe par la commande /f.

local POWER_MAX = 10        -- puissance d'un joueur au plus
local POWER_DEATH = 4       -- puissance perdue a chaque mort
local POWER_EVERY = 300     -- secondes pour regagner un point de puissance, en jeu
local SAFE = 1              -- la zone sure fait (2 x SAFE + 1) chunks de cote autour du point de depart
local HOME_DELAY = 5        -- secondes d'attente, sans bouger, avant d'etre ramene a la base
local NAME_MAX = 16

rules.set("pvp", true)

-- ------------------------------------------------------------------------------------------------
-- Ce que le mod retient, enregistre avec le monde.
local factions = {}         -- nom -> { leader, members = { pseudo = true }, home = { x, y, z }, allies = {}, enemies = {}, invites = {} }
local claims = {}           -- "cx,cz" -> nom de la faction
local power = {}            -- pseudo -> puissance
local faction_of = {}       -- pseudo -> nom de sa faction
local spawn                 -- chunk du point de depart : le centre de la zone sure

local function split(text, sep)
  local out = {}
  for piece in string.gmatch(text or "", "([^" .. sep .. "]+)") do out[#out + 1] = piece end
  return out
end

local function keys(t)
  local out = {}
  for k in pairs(t) do out[#out + 1] = k end
  table.sort(out)
  return out
end

local function save_faction(name)
  local f = factions[name]
  if not f then
    store.set("f:" .. name, "")
  else
    local home = f.home and (f.home[1] .. "," .. f.home[2] .. "," .. f.home[3]) or "-"
    store.set("f:" .. name, table.concat({ f.leader, home, table.concat(keys(f.members), ","), table.concat(keys(f.allies), ","), table.concat(keys(f.enemies), ",") }, "|"))
  end
  store.set("factions", table.concat(keys(factions), ";"))
end

local function save_claims(name)
  local mine = {}
  for key, owner in pairs(claims) do if owner == name then mine[#mine + 1] = key end end
  table.sort(mine)
  store.set("c:" .. name, table.concat(mine, ";"))
end

local function load()
  for _, name in ipairs(split(store.get("factions"), ";")) do
    local parts = split((store.get("f:" .. name) or "") .. "|", "|")
    if parts[1] and parts[1] ~= "" then
      local f = { leader = parts[1], members = {}, allies = {}, enemies = {}, invites = {} }
      local h = split(parts[2] or "-", ",")
      if #h == 3 then f.home = { tonumber(h[1]), tonumber(h[2]), tonumber(h[3]) } end
      for _, m in ipairs(split(parts[3], ",")) do f.members[m], faction_of[m] = true, name end
      for _, a in ipairs(split(parts[4], ",")) do f.allies[a] = true end
      for _, e in ipairs(split(parts[5], ",")) do f.enemies[e] = true end
      factions[name] = f
      for _, key in ipairs(split(store.get("c:" .. name), ";")) do claims[key] = name end
    end
  end
  local s = split(store.get("spawn"), ",")
  if #s == 2 then spawn = { tonumber(s[1]), tonumber(s[2]) } end
end
load()

-- ------------------------------------------------------------------------------------------------
-- Terres et puissance.
local function chunk_of(x, z) return math.floor(x / 16), math.floor(z / 16) end

local function here(p)
  local x, _, z = p:pos()
  if not x then return nil end
  local cx, cz = chunk_of(x, z)
  return cx, cz, cx .. "," .. cz
end

local function safe(cx, cz)
  return spawn ~= nil and math.abs(cx - spawn[1]) <= SAFE and math.abs(cz - spawn[2]) <= SAFE
end

local function power_of(name)
  if power[name] == nil then power[name] = tonumber(store.get("p:" .. name) or "") or POWER_MAX end
  return power[name]
end

local function set_power(name, value)
  power[name] = math.max(0, math.min(POWER_MAX, value))
  store.set("p:" .. name, tostring(power[name]))
end

local function faction_power(name)
  local total = 0
  for member in pairs(factions[name].members) do total = total + power_of(member) end
  return total
end

local function land(name)
  local n = 0
  for _, owner in pairs(claims) do if owner == name then n = n + 1 end end
  return n
end

local function size(name)
  local n = 0
  for _ in pairs(factions[name].members) do n = n + 1 end
  return n
end

-- Ce que deux factions sont l'une pour l'autre.
local function relation(a, b)
  if not a or not b then return "neutre" end
  if a == b then return "membre" end
  if factions[a].enemies[b] or factions[b].enemies[a] then return "ennemi" end
  if factions[a].allies[b] and factions[b].allies[a] then return "allie" end
  return "neutre"
end

local function territory(cx, cz)
  if safe(cx, cz) then return "Zone sure" end
  local owner = claims[cx .. "," .. cz]
  return owner and ("Terres de " .. owner) or "Zone libre"
end

-- Le joueur a-t-il le droit de toucher a ce bloc ?
local function allowed(p, x, z)
  local cx, cz = chunk_of(x, z)
  if safe(cx, cz) then return false, "La zone sure ne se modifie pas." end
  local owner = claims[cx .. "," .. cz]
  if owner and owner ~= faction_of[p.name] then return false, "Ces terres sont a la faction " .. owner .. "." end
  return true
end

local function tell_faction(name, text)
  for _, p in ipairs(players()) do
    if faction_of[p.name] == name then p:tell(text) end
  end
end

local function named(name)
  for _, p in ipairs(players()) do if p.name:lower() == name:lower() then return p end end
end

-- ------------------------------------------------------------------------------------------------
-- La commande /f.
local HELP = {
  "/f creer <nom> : fonde une faction",
  "/f inviter <joueur>, /f rejoindre <faction>, /f quitter, /f exclure <joueur>",
  "/f claim : revendique le chunk ou l'on se tient ; /f unclaim : le rend",
  "/f sethome, /f home : la base de la faction",
  "/f allie <faction>, /f ennemi <faction>, /f neutre <faction>",
  "/f info [faction], /f liste, /f carte, /f power",
  "/f chef <joueur> : passe le commandement ; /f dissoudre",
  "/f c <message> : parle a sa faction",
}

local sub = {}

function sub.aide(p)
  for _, line in ipairs(HELP) do p:tell(line) end
  return "Puissance : " .. POWER_MAX .. " au plus par joueur, " .. POWER_DEATH .. " perdus a chaque mort, 1 regagne toutes les " .. math.floor(POWER_EVERY / 60) .. " minutes."
end

function sub.creer(p, name)
  if faction_of[p.name] then return "Vous etes deja dans la faction " .. faction_of[p.name] .. "." end
  if not name or #name < 3 or #name > NAME_MAX or not name:match("^[%w_]+$") then return "Un nom de 3 a " .. NAME_MAX .. " lettres, chiffres ou tirets bas." end
  for other in pairs(factions) do if other:lower() == name:lower() then return "Une faction porte deja ce nom." end end
  factions[name] = { leader = p.name, members = { [p.name] = true }, allies = {}, enemies = {}, invites = {} }
  faction_of[p.name] = name
  save_faction(name)
  say(p.name .. " fonde la faction " .. name .. ".")
  return "Faction " .. name .. " fondee. /f claim revendique le chunk ou vous etes."
end

function sub.inviter(p, who)
  local mine = faction_of[p.name]
  if not mine then return "Vous n'avez pas de faction." end
  local guest = who and named(who)
  if not guest then return "Ce joueur n'est pas en ligne." end
  if faction_of[guest.name] then return guest.name .. " a deja une faction." end
  factions[mine].invites[guest.name] = true
  guest:tell(p.name .. " vous invite dans la faction " .. mine .. " : /f rejoindre " .. mine)
  return guest.name .. " est invite."
end

function sub.rejoindre(p, name)
  if faction_of[p.name] then return "Quittez d'abord votre faction (/f quitter)." end
  local f = name and factions[name]
  if not f then return "Cette faction n'existe pas." end
  if not f.invites[p.name] then return "Il faut y avoir ete invite." end
  f.invites[p.name] = nil
  f.members[p.name], faction_of[p.name] = true, name
  save_faction(name)
  tell_faction(name, p.name .. " rejoint la faction.")
  return "Bienvenue dans la faction " .. name .. "."
end

local function disband(name)
  for key, owner in pairs(claims) do if owner == name then claims[key] = nil end end
  for member in pairs(factions[name].members) do faction_of[member] = nil end
  for _, f in pairs(factions) do f.allies[name], f.enemies[name] = nil, nil end
  factions[name] = nil
  save_faction(name)
  save_claims(name)
  say("La faction " .. name .. " est dissoute.")
end

function sub.quitter(p)
  local mine = faction_of[p.name]
  if not mine then return "Vous n'avez pas de faction." end
  local f = factions[mine]
  if f.leader == p.name then
    if size(mine) > 1 then return "Le chef passe d'abord le commandement (/f chef <joueur>) ou dissout la faction (/f dissoudre)." end
    disband(mine)
    return "Vous etiez le dernier : la faction est dissoute."
  end
  f.members[p.name], faction_of[p.name] = nil, nil
  save_faction(mine)
  tell_faction(mine, p.name .. " quitte la faction.")
  return "Vous quittez la faction " .. mine .. "."
end

function sub.exclure(p, who)
  local mine = faction_of[p.name]
  if not mine or factions[mine].leader ~= p.name then return "Seul le chef exclut un membre." end
  if not who or not factions[mine].members[who] or who == p.name then return "Ce joueur n'est pas un autre membre de votre faction." end
  factions[mine].members[who], faction_of[who] = nil, nil
  save_faction(mine)
  tell_faction(mine, who .. " est exclu de la faction.")
  return who .. " est exclu."
end

function sub.chef(p, who)
  local mine = faction_of[p.name]
  if not mine or factions[mine].leader ~= p.name then return "Seul le chef passe le commandement." end
  if not who or not factions[mine].members[who] then return "Ce joueur n'est pas de votre faction." end
  factions[mine].leader = who
  save_faction(mine)
  tell_faction(mine, who .. " est le nouveau chef de la faction.")
  return "Commandement passe a " .. who .. "."
end

function sub.dissoudre(p)
  local mine = faction_of[p.name]
  if not mine or factions[mine].leader ~= p.name then return "Seul le chef dissout la faction." end
  disband(mine)
  return "Faction dissoute."
end

function sub.claim(p)
  local mine = faction_of[p.name]
  if not mine then return "Il faut une faction pour revendiquer des terres (/f creer <nom>)." end
  local cx, cz, key = here(p)
  if not cx then return "Position inconnue." end
  if safe(cx, cz) then return "La zone sure n'appartient a personne." end
  local owner = claims[key]
  if owner == mine then return "Ce chunk est deja a vous." end
  if land(mine) + 1 > faction_power(mine) then
    return "Pas assez de puissance : " .. land(mine) .. " terres pour " .. faction_power(mine) .. " de puissance."
  end
  if owner then
    -- Une faction qui tient plus de terres qu'elle n'a de puissance peut se les faire prendre.
    if land(owner) <= faction_power(owner) then return "Ces terres sont a " .. owner .. ", qui a la puissance de les tenir." end
    if relation(mine, owner) == "allie" then return "On ne prend pas les terres d'un allie." end
    tell_faction(owner, "La faction " .. mine .. " vous prend des terres en " .. cx .. ", " .. cz .. " !")
    if factions[owner].home then
      local hx, hz = chunk_of(factions[owner].home[1], factions[owner].home[3])
      if hx == cx and hz == cz then factions[owner].home = nil save_faction(owner) end
    end
  end
  claims[key] = mine
  save_claims(mine)
  if owner then save_claims(owner) end
  tell_faction(mine, p.name .. " revendique le chunk " .. cx .. ", " .. cz .. " (" .. land(mine) .. " / " .. faction_power(mine) .. ").")
  return "Chunk revendique."
end

function sub.unclaim(p)
  local mine = faction_of[p.name]
  local cx, cz, key = here(p)
  if not mine or not cx or claims[key] ~= mine then return "Ce chunk n'est pas a votre faction." end
  claims[key] = nil
  save_claims(mine)
  return "Chunk rendu."
end

function sub.sethome(p)
  local mine = faction_of[p.name]
  local cx, cz, key = here(p)
  if not mine or not cx or claims[key] ~= mine then return "La base se place sur vos terres." end
  local x, y, z = p:pos()
  factions[mine].home = { math.floor(x) + 0.5, math.floor(y * 10) / 10, math.floor(z) + 0.5 }
  save_faction(mine)
  tell_faction(mine, p.name .. " fixe la base de la faction ici.")
  return "Base fixee."
end

local going = {}            -- pseudo -> { secondes restantes, x, z de depart }

function sub.home(p)
  local mine = faction_of[p.name]
  if not mine or not factions[mine].home then return "Votre faction n'a pas de base (/f sethome)." end
  local x, _, z = p:pos()
  going[p.name] = { HOME_DELAY, x, z }
  return "Retour a la base dans " .. HOME_DELAY .. " secondes : ne bougez pas."
end

local function set_relation(p, other, kind)
  local mine = faction_of[p.name]
  if not mine or factions[mine].leader ~= p.name then return "Seul le chef decide des relations." end
  if not other or not factions[other] or other == mine then return "Cette faction n'existe pas." end
  local f = factions[mine]
  f.allies[other], f.enemies[other] = nil, nil
  if kind == "allie" then f.allies[other] = true elseif kind == "ennemi" then f.enemies[other] = true end
  save_faction(mine)
  local now = relation(mine, other)
  if kind == "allie" and now ~= "allie" then
    tell_faction(other, "La faction " .. mine .. " vous propose une alliance : /f allie " .. mine)
    return "Alliance proposee a " .. other .. " : elle doit l'accepter."
  end
  say("Les factions " .. mine .. " et " .. other .. " sont desormais : " .. now .. ".")
  return "Relation avec " .. other .. " : " .. now .. "."
end
function sub.allie(p, other) return set_relation(p, other, "allie") end
function sub.ennemi(p, other) return set_relation(p, other, "ennemi") end
function sub.neutre(p, other) return set_relation(p, other, "neutre") end

function sub.info(p, name)
  name = name or faction_of[p.name]
  local f = name and factions[name]
  if not f then return "Faction inconnue. /f liste les montre toutes." end
  p:tell("Faction " .. name .. " - chef : " .. f.leader .. " - " .. size(name) .. " membre(s) : " .. table.concat(keys(f.members), ", "))
  p:tell("Terres : " .. land(name) .. " - puissance : " .. faction_power(name) .. " / " .. size(name) * POWER_MAX)
  local allies, enemies = {}, {}
  for other in pairs(factions) do
    local r = relation(name, other)
    if r == "allie" then allies[#allies + 1] = other elseif r == "ennemi" then enemies[#enemies + 1] = other end
  end
  return "Allies : " .. (#allies > 0 and table.concat(allies, ", ") or "aucun") .. " - ennemis : " .. (#enemies > 0 and table.concat(enemies, ", ") or "aucun")
end

function sub.liste(p)
  local names = keys(factions)
  if #names == 0 then return "Aucune faction pour le moment : /f creer <nom>." end
  for _, name in ipairs(names) do
    p:tell(name .. " - " .. size(name) .. " membre(s) - terres " .. land(name) .. " - puissance " .. faction_power(name))
  end
  return #names .. " faction(s)."
end

function sub.power(p)
  local mine = faction_of[p.name]
  local text = "Votre puissance : " .. power_of(p.name) .. " / " .. POWER_MAX
  if mine then text = text .. " - faction : " .. faction_power(mine) .. " pour " .. land(mine) .. " terres" end
  return text
end

-- Les chunks alentour : + vous, # les votres, A un allie, X un ennemi, o une autre faction, S la zone sure.
function sub.carte(p)
  local cx, cz = here(p)
  if not cx then return "Position inconnue." end
  local mine = faction_of[p.name]
  for dz = -4, 4 do
    local line = ""
    for dx = -6, 6 do
      local owner = claims[(cx + dx) .. "," .. (cz + dz)]
      local sign = "-"
      if dx == 0 and dz == 0 then sign = "+"
      elseif safe(cx + dx, cz + dz) then sign = "S"
      elseif owner then
        local r = relation(mine, owner)
        sign = r == "membre" and "#" or r == "allie" and "A" or r == "ennemi" and "X" or "o"
      end
      line = line .. sign .. " "
    end
    p:tell(line)
  end
  return "+ vous   # vos terres   A allie   X ennemi   o autre faction   S zone sure"
end

function sub.c(p, _, rest)
  local mine = faction_of[p.name]
  if not mine then return "Vous n'avez pas de faction." end
  if rest == "" then return "/f c <message>" end
  tell_faction(mine, "[" .. mine .. "] " .. p.name .. " : " .. rest)
end

command("f", "Factions : /f aide pour tout savoir", function(p, args)
  local words = split(args or "", " ")
  local name = words[1] or "aide"
  local fn = sub[name]
  if not fn then return "Commande inconnue : /f " .. name .. ". Tapez /f aide." end
  local rest = (args or ""):sub(#name + 2)
  return fn(p, words[2], rest)
end)

-- ------------------------------------------------------------------------------------------------
-- La vie du monde.
on("join", function(p)
  p:set_mode("survie")
  if not spawn then
    -- Le premier joueur arrive au point de depart du monde : la zone sure s'etend autour.
    local x, _, z = p:pos()
    if x then
      spawn = { chunk_of(x, z) }
      store.set("spawn", spawn[1] .. "," .. spawn[2])
    end
  end
  local mine = faction_of[p.name]
  p:tell(mine and ("Faction " .. mine .. " - /f aide pour les commandes.") or "Bienvenue ! /f creer <nom> fonde votre faction, /f aide dit tout.")
end)

on("break", function(p, x, y, z)
  local ok, why = allowed(p, x, z)
  if not ok then p:info(why) return false end
end)
on("place", function(p, x, y, z)
  local ok, why = allowed(p, x, z)
  if not ok then p:info(why) return false end
end)
-- Chez les autres, on n'ouvre ni coffre ni porte et l'on n'actionne rien.
on("interact", function(p, x, y, z)
  local cx, cz = chunk_of(x, z)
  local owner = claims[cx .. "," .. cz]
  if owner and owner ~= faction_of[p.name] and relation(faction_of[p.name], owner) ~= "allie" then
    p:info("Ces terres sont a la faction " .. owner .. ".")
    return false
  end
end)

-- Pas de coup entre membres ou allies, ni dans la zone sure.
on("hit", function(p, victim)
  local r = relation(faction_of[p.name], faction_of[victim])
  if r == "membre" or r == "allie" then
    p:info(victim .. " est de votre camp.")
    return false
  end
  local cx, cz = here(p)
  local other = named(victim)
  local ox, oz
  if other then ox, oz = here(other) end
  if (cx and safe(cx, cz)) or (ox and safe(ox, oz)) then
    p:info("On ne se bat pas dans la zone sure.")
    return false
  end
end)

on("death", function(p, killer)
  set_power(p.name, power_of(p.name) - POWER_DEATH)
  p:tell("Vous perdez " .. POWER_DEATH .. " de puissance (" .. power_of(p.name) .. " / " .. POWER_MAX .. ").")
  local mine = faction_of[p.name]
  if mine and land(mine) > faction_power(mine) then
    tell_faction(mine, "Attention : la faction tient " .. land(mine) .. " terres pour " .. faction_power(mine) .. " de puissance. On peut vous en prendre !")
  end
end)

local ticks, where, online = 0, {}, {}
on("tick", function()
  ticks = ticks + 1
  if ticks % 20 ~= 0 then return end
  for _, p in ipairs(players()) do
    local cx, cz = here(p)
    if cx then
      -- On previent le joueur quand il change de territoire.
      local now = territory(cx, cz)
      if where[p.name] ~= now then
        if where[p.name] then p:info("~ " .. now .. " ~") end
        where[p.name] = now
      end
      -- La puissance revient avec le temps passe en jeu.
      online[p.name] = (online[p.name] or 0) + 1
      if online[p.name] >= POWER_EVERY then
        online[p.name] = 0
        if power_of(p.name) < POWER_MAX then
          set_power(p.name, power_of(p.name) + 1)
          p:tell("Votre puissance remonte : " .. power_of(p.name) .. " / " .. POWER_MAX .. ".")
        end
      end
      -- Retour a la base : il faut etre reste sur place.
      local go = going[p.name]
      if go then
        local x, _, z = p:pos()
        if math.abs(x - go[2]) > 0.5 or math.abs(z - go[3]) > 0.5 then
          going[p.name] = nil
          p:info("Vous avez bouge : retour annule.")
        else
          go[1] = go[1] - 1
          if go[1] <= 0 then
            going[p.name] = nil
            local home = faction_of[p.name] and factions[faction_of[p.name]].home
            if home then p:teleport(home[1], home[2] + 0.2, home[3]) end
          end
        end
      end
      local mine = faction_of[p.name]
      local head = mine and (mine .. " - terres " .. land(mine) .. " / " .. faction_power(mine)) or "Sans faction (/f creer)"
      p:send("etat", head .. " - puissance " .. power_of(p.name) .. " / " .. POWER_MAX .. " - " .. now)
    end
  end
end)
