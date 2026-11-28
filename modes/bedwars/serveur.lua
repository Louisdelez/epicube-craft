-- Bed Wars : quatre iles dans le ciel, un lit par equipe. Tant que son lit tient, une equipe revient
-- en jeu apres chaque mort (au bout de quelques secondes) ; le lit detruit, elle n'a plus qu'une vie.
-- Chaque ile forge du fer et de l'or ; les ilots de diamant et le centre, ses cristaux de braise, se
-- disputent. La partie se durcit avec le temps : diamants plus rapides, puis tous les lits detruits,
-- puis la mort subite. La derniere equipe gagne.

local MIN_PLAYERS = 2
local COUNTDOWN = 15
local HEIGHT = 250          -- altitude des iles
local DISTANCE = 26         -- du centre a chaque ile
local RESPAWN = 5           -- secondes d'attente avant de revenir en jeu
local DIAMOND_II = 360      -- secondes : les diamants tombent deux fois plus vite
local BEDS_GONE = 600       -- secondes : tous les lits sont detruits
local SUDDEN = 900          -- secondes : mort subite
local TEAMS = {
  { id = "rouge", name = "Rouge", dx = 1, dz = 0 },
  { id = "bleu", name = "Bleue", dx = -1, dz = 0 },
  { id = "vert", name = "Verte", dx = 0, dz = 1 },
  { id = "jaune", name = "Jaune", dx = 0, dz = -1 },
}
-- Ce que vend la boutique : objet, quantite, monnaie, prix. Le script client affiche la meme liste.
local SHOP = {
  { "fourrure", 16, "lingot_de_fer", 4 },
  { "planches_de_hinoki", 8, "lingot_de_fer", 6 },
  { "katana_en_pierre", 1, "lingot_de_fer", 10 },
  { "pioche_en_bois", 1, "lingot_de_fer", 8 },
  { "onigiri", 4, "lingot_de_fer", 3 },
  { "katana_en_fer", 1, "diamant", 3 },
  { "cuirasse_do_de_fer", 1, "diamant", 4 },
  { "obsidienne", 4, "diamant", 4 },
  { "pierre_de_muraille", 12, "lingot_d_or", 4 },
  { "pioche_en_fer", 1, "lingot_d_or", 6 },
  { "arc_yumi", 1, "lingot_d_or", 12 },
  { "fleche", 8, "lingot_d_or", 2 },
  { "bouclier_tate", 1, "lingot_d_or", 5 },
  { "feux_d_artifice", 1, "lingot_d_or", 8 },
  { "pomme", 2, "lingot_d_or", 3 },
  { "katana_en_diamant", 1, "cristal_de_braise", 4 },
  { "casque_kabuto_de_fer", 1, "cristal_de_braise", 2 },
  { "jambieres_suneate_de_fer", 1, "cristal_de_braise", 2 },
}
-- Amelioration d'equipe : la forge de l'ile produit deux fois plus vite. Elle se paie en diamants.
local FORGE_PRICE = 6

for _, t in ipairs(TEAMS) do
  blocks.register{ id = "lit_" .. t.id, name = "Lit de l'equipe " .. t.name, hardness = 0.6, drop = false }
end
blocks.register{ id = "boutique", name = "Boutique", hardness = -1 }
blocks.register{ id = "generateur", name = "Generateur", hardness = -1, light = 8 }

rules.set("mobs", false)
rules.set("pvp", false)

local phase = "attente"     -- attente, jeu, fin
local wait, ticks, clock = COUNTDOWN, 0, 0
local ox, oz = 0, 0         -- centre de l'arene
local team_of = {}          -- pseudo -> numero d'equipe
local bed = {}              -- numero d'equipe -> son lit tient encore
local out = {}              -- pseudo -> elimine
local placed = {}           -- blocs poses par les joueurs : eux seuls se cassent
local used = 0              -- nombre d'equipes de la partie
local forge = {}            -- numero d'equipe -> forge amelioree
local back = {}             -- pseudo -> secondes avant de revenir en jeu
local kills = {}

local function key(x, y, z) return x .. "," .. y .. "," .. z end
local function island(i) return ox + TEAMS[i].dx * DISTANCE, oz + TEAMS[i].dz * DISTANCE end

-- Les ilots de diamant, entre les iles des equipes.
local function gem(k)
  local d = math.floor(DISTANCE * 0.7)
  local sx, sz = (k == 1 or k == 4) and 1 or -1, (k <= 2) and 1 or -1
  return ox + sx * d, oz + sz * d
end

-- Une ile : son plateau, et un socle qui s'effile vers le bas.
local function slab(x, z, r)
  world.fill(x - r, HEIGHT - 1, z - r, x + r, HEIGHT, z + r, "pierre")
  world.fill(x - r + 1, HEIGHT - 2, z - r + 1, x + r - 1, HEIGHT - 2, z + r - 1, "pierre")
  if r > 2 then world.fill(x - r + 2, HEIGHT - 3, z - r + 2, x + r - 2, HEIGHT - 3, z + r - 2, "pierre") end
end

local function build()
  -- Le centre : une grande ile et son generateur de cristaux de braise.
  slab(ox, oz, 5)
  world.set(ox, HEIGHT, oz, "bedwars:generateur")
  for k = 1, 4 do
    local gx, gz = gem(k)
    slab(gx, gz, 2)
    world.set(gx, HEIGHT, gz, "bedwars:generateur")
  end
  for i = 1, used do
    local ix, iz = island(i)
    local t = TEAMS[i]
    slab(ix, iz, 4)
    world.set(ix, HEIGHT, iz, "bedwars:generateur")
    -- Le lit, du cote oppose au centre ; la boutique a cote.
    world.set(ix + t.dx * 3, HEIGHT + 1, iz + t.dz * 3, "bedwars:lit_" .. t.id)
    world.set(ix + t.dz * 3, HEIGHT + 1, iz + t.dx * 3, "bedwars:boutique")
    bed[i] = true
  end
end

local function members(i)
  local n = 0
  for name, t in pairs(team_of) do if t == i and not out[name] then n = n + 1 end end
  return n
end

local function status(p)
  if phase == "attente" then
    local n = #players()
    if n < MIN_PLAYERS then return "Bed Wars - en attente de joueurs (" .. n .. "/" .. MIN_PLAYERS .. ")" end
    return "Bed Wars - depart dans " .. wait .. " s"
  end
  local parts = {}
  for i = 1, used do
    local n = members(i)
    parts[#parts + 1] = TEAMS[i].name .. " " .. (bed[i] and "lit" or "sans lit") .. " " .. n
  end
  local mine = team_of[p.name] and (" - vous : " .. TEAMS[team_of[p.name]].name) or ""
  -- Ce qui vient : les diamants, les lits, la mort subite.
  local next_at, what = DIAMOND_II, "diamants II"
  if clock >= DIAMOND_II then next_at, what = BEDS_GONE, "fin des lits" end
  if clock >= BEDS_GONE then next_at, what = SUDDEN, "mort subite" end
  local soon = clock < SUDDEN and (" - " .. what .. " dans " .. (next_at - clock) .. " s") or " - mort subite"
  local wait_text = back[p.name] and (" - retour dans " .. back[p.name] .. " s") or ""
  return table.concat(parts, " | ") .. mine .. soon .. wait_text
end

local function kit(p)
  p:give("bokken", 1)
  p:give("fourrure", 8)
end

local function send_home(p)
  local ix, iz = island(team_of[p.name])
  p:teleport(ix + 0.5, HEIGHT + 2, iz + 0.5)
end

local function check_end()
  local standing, last = 0, nil
  for i = 1, used do
    if members(i) > 0 then standing, last = standing + 1, i end
  end
  if standing <= 1 and phase == "jeu" then
    phase = "fin"
    rules.set("pvp", false)
    local text = last and ("L'equipe " .. TEAMS[last].name .. " remporte la partie !") or "Partie terminee."
    announce(text)
    say(text)
  end
end

local function start()
  local list = players()
  local x, _, z = list[1]:pos()
  ox, oz = math.floor(x), math.floor(z)
  used = math.min(#TEAMS, math.max(2, #list))
  build()
  team_of, out, placed, forge, back, kills = {}, {}, {}, {}, {}, {}
  for i, p in ipairs(list) do
    team_of[p.name] = (i - 1) % used + 1
    local ix, iz = island(team_of[p.name])
    p:clear()
    p:set_mode("survie")
    p:set_health(20)
    p:set_hunger(20)
    p:set_home(ix + 0.5, HEIGHT + 2, iz + 0.5)
    send_home(p)
    kit(p)
    p:tell("Vous etes dans l'equipe " .. TEAMS[team_of[p.name]].name .. ". Clic droit sur la boutique pour acheter.")
  end
  phase, clock = "jeu", 0
  rules.set("pvp", true)
  announce("Bed Wars : c'est parti !")
end

on("join", function(p)
  if phase == "attente" then
    p:set_mode("aventure")
    p:tell("Bienvenue dans Bed Wars ! La partie commence des que " .. MIN_PLAYERS .. " joueurs sont la.")
  else
    p:set_mode("spectateur")
    p:tell("La partie est en cours : vous la regardez en spectateur.")
  end
end)

on("leave", function(p)
  if team_of[p.name] then
    out[p.name] = true
    check_end()
  end
end)

on("death", function(p, killer)
  local t = team_of[p.name]
  if phase ~= "jeu" or not t then return end
  if killer ~= "" and killer then kills[killer] = (kills[killer] or 0) + 1 end
  if not bed[t] then
    out[p.name] = true
    say(p.name .. " (equipe " .. TEAMS[t].name .. ") est elimine" .. ((killer ~= "" and killer) and (" par " .. killer) or "") .. ".")
    check_end()
  end
end)

on("respawn", function(p)
  if phase == "attente" then return end
  if out[p.name] or not team_of[p.name] then
    p:set_mode("spectateur")
  else
    -- Le lit tient : on attend quelques secondes au-dessus de son ile, puis on revient en jeu.
    local ix, iz = island(team_of[p.name])
    p:set_mode("spectateur")
    p:teleport(ix + 0.5, HEIGHT + 10, iz + 0.5)
    back[p.name] = RESPAWN
    p:info("Retour en jeu dans " .. RESPAWN .. " secondes.")
  end
end)

-- Seuls se cassent les blocs poses par les joueurs, et les lits des autres equipes.
on("break", function(p, x, y, z, block)
  if phase ~= "jeu" then return false end
  for i = 1, used do
    if block == "bedwars:lit_" .. TEAMS[i].id then
      if team_of[p.name] == i then
        p:info("C'est votre lit !")
        return false
      end
      bed[i] = false
      local text = "Le lit de l'equipe " .. TEAMS[i].name .. " est detruit par " .. p.name .. " !"
      announce(text)
      say(text)
      world.sound("explosion", x, y, z, 1)
      return true
    end
  end
  local k = key(x, y, z)
  if placed[k] then
    placed[k] = nil
    return true
  end
  return false
end)

on("place", function(p, x, y, z)
  if phase ~= "jeu" or y > HEIGHT + 24 or y < HEIGHT - 12 or math.abs(x - ox) > DISTANCE + 14 or math.abs(z - oz) > DISTANCE + 14 then
    return false
  end
  placed[key(x, y, z)] = true
end)

-- La boutique s'ouvre d'un clic droit ; le script client envoie ensuite l'article voulu.
on("interact", function(p, x, y, z, block)
  if block == "bedwars:boutique" then
    p:send("boutique", "")
    return false
  end
end)

on("message", function(p, name, data)
  if name == "forge" and phase == "jeu" and team_of[p.name] then
    local t = team_of[p.name]
    if forge[t] then
      p:send("achat", "deja")
    elseif p:take("diamant", FORGE_PRICE) then
      forge[t] = true
      p:sound("clic", 0.8)
      p:send("achat", "ok")
      say("L'equipe " .. TEAMS[t].name .. " ameliore sa forge : le fer et l'or y tombent deux fois plus vite.")
    else
      p:send("achat", "non")
    end
    return
  end
  if name ~= "acheter" or phase ~= "jeu" then return end
  local item = SHOP[tonumber(data) or 0]
  if not item then return end
  if p:take(item[3], item[4]) then
    p:give(item[1], item[2])
    p:sound("clic", 0.8)
    p:send("achat", "ok")
  else
    p:send("achat", "non")
  end
end)

on("tick", function()
  ticks = ticks + 1
  if ticks % 20 ~= 0 then return end
  local list = players()
  if phase == "attente" then
    if #list >= MIN_PLAYERS then
      wait = wait - 1
      if wait <= 0 then start() elseif wait <= 5 or wait % 5 == 0 then announce("Depart dans " .. wait .. " s") end
    else
      wait = COUNTDOWN
    end
  elseif phase == "jeu" then
    clock = clock + 1
    -- Les forges des iles : du fer souvent, de l'or de temps en temps ; deux fois plus vite une fois ameliorees.
    for i = 1, used do
      local ix, iz = island(i)
      local fast = forge[i] and 2 or 1
      if clock % (2 / fast) == 0 then world.drop("lingot_de_fer", 1, ix + 0.5, HEIGHT + 1.3, iz + 0.5) end
      if clock % (8 / fast) == 0 then world.drop("lingot_d_or", 1, ix + 0.5, HEIGHT + 1.3, iz + 0.5) end
    end
    -- Les ilots de diamant, puis le centre et ses cristaux de braise.
    if clock % (clock >= DIAMOND_II and 10 or 20) == 0 then
      for k = 1, 4 do
        local gx, gz = gem(k)
        world.drop("diamant", 1, gx + 0.5, HEIGHT + 1.3, gz + 0.5)
      end
    end
    if clock % 30 == 0 then world.drop("cristal_de_braise", 1, ox + 0.5, HEIGHT + 1.3, oz + 0.5) end
    -- La partie se durcit.
    if clock == DIAMOND_II then announce("Diamants II : ils tombent deux fois plus vite.") end
    if clock == BEDS_GONE then
      for i = 1, used do
        if bed[i] then
          local ix, iz = island(i)
          world.set(ix + TEAMS[i].dx * 3, HEIGHT + 1, iz + TEAMS[i].dz * 3, "air")
          bed[i] = false
        end
      end
      announce("Tous les lits sont detruits : plus de retour en jeu !")
      say("Tous les lits sont detruits : plus de retour en jeu !")
    end
    if clock == SUDDEN then announce("Mort subite !") end
    -- Ceux qui attendent de revenir en jeu.
    for _, p in ipairs(list) do
      local left = back[p.name]
      if left then
        if left <= 1 then
          back[p.name] = nil
          if not out[p.name] then
            p:set_mode("survie")
            p:set_health(20)
            p:set_hunger(20)
            send_home(p)
            kit(p)
          end
        else
          back[p.name] = left - 1
        end
      end
    end
    -- Tomber de l'ile, c'est mourir.
    for _, p in ipairs(list) do
      local _, y = p:pos()
      local playing = team_of[p.name] and not out[p.name] and not back[p.name]
      if y and y < HEIGHT - 30 and playing then p:damage(1000) elseif playing and clock > SUDDEN and clock % 3 == 0 then p:damage(1) end
    end
  end
  for _, p in ipairs(list) do p:send("etat", status(p)) end
end)

command("bedwars", "Lance la partie sans attendre (avec « demarrer »), ou dit ou elle en est", function(p, args)
  if args == "demarrer" and phase == "attente" then
    start()
    return "Bed Wars lance."
  end
  return status(p)
end)
