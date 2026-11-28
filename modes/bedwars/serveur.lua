-- Bed Wars : quatre iles dans le ciel, un lit par equipe. Tant que son lit tient, une equipe revient
-- en jeu apres chaque mort ; le lit detruit, elle n'a plus qu'une vie. La derniere equipe gagne.

local MIN_PLAYERS = 2
local COUNTDOWN = 15
local HEIGHT = 250          -- altitude des iles
local DISTANCE = 26         -- du centre a chaque ile
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
}

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

local function key(x, y, z) return x .. "," .. y .. "," .. z end
local function island(i) return ox + TEAMS[i].dx * DISTANCE, oz + TEAMS[i].dz * DISTANCE end

local function build()
  -- Le centre : une grande ile et son generateur de diamants.
  world.fill(ox - 5, HEIGHT, oz - 5, ox + 5, HEIGHT, oz + 5, "pierre")
  world.set(ox, HEIGHT, oz, "bedwars:generateur")
  for i = 1, used do
    local ix, iz = island(i)
    local t = TEAMS[i]
    world.fill(ix - 4, HEIGHT - 1, iz - 4, ix + 4, HEIGHT, iz + 4, "pierre")
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
  return table.concat(parts, " | ") .. mine
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
  team_of, out, placed = {}, {}, {}
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
    send_home(p)
    kit(p)
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
    -- Les generateurs : du fer sur chaque ile, des diamants au centre.
    if clock % 2 == 0 then
      for i = 1, used do
        local ix, iz = island(i)
        world.drop("lingot_de_fer", 1, ix + 0.5, HEIGHT + 1.3, iz + 0.5)
      end
    end
    if clock % 20 == 0 then world.drop("diamant", 1, ox + 0.5, HEIGHT + 1.3, oz + 0.5) end
    -- Tomber de l'ile, c'est mourir.
    for _, p in ipairs(list) do
      local _, y = p:pos()
      if y and y < HEIGHT - 30 and team_of[p.name] and not out[p.name] then p:damage(1000) end
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
