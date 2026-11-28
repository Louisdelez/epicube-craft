-- Sky Wars : huit iles autour d'une ile centrale, haut dans le ciel. Chaque joueur attend dans une cage
-- au-dessus de la sienne ; au depart les cages s'ouvrent, on vide ses coffres, on batit un pont vers les
-- autres ou vers le centre, mieux garni. Tomber dans le vide tue, il n'y a pas de seconde vie : le dernier
-- debout gagne. Les coffres se regarnissent une fois en cours de partie.

local MIN_PLAYERS = 2
local COUNTDOWN = 15        -- secondes d'attente une fois assez de joueurs reunis
local CAGE = 8              -- secondes passees dans la cage avant qu'elle s'ouvre
local HEIGHT = 250          -- altitude des iles
local RADIUS = 34           -- du centre a chaque ile
local ISLANDS = 8
local REFILL = 180          -- secondes avant que les coffres se regarnissent
local SUDDEN = 480          -- secondes avant la mort subite

-- Butin des iles (de quoi s'armer et batir) et du centre (de quoi gagner) : objet, minimum, maximum, chances sur 10.
local LOOT = {
  { "planches_de_hinoki", 12, 24, 10 }, { "pierre_de_muraille", 8, 20, 8 }, { "fourrure", 8, 16, 6 },
  { "katana_en_pierre", 1, 1, 6 }, { "bokken", 1, 1, 5 }, { "hache_ono_en_pierre", 1, 1, 4 }, { "pioche_en_pierre", 1, 1, 5 },
  { "onigiri", 2, 5, 8 }, { "viande_cuite", 1, 3, 5 }, { "fleche", 4, 10, 4 }, { "arc_yumi", 1, 1, 3 },
  { "kunai", 2, 5, 4 }, { "bouclier_tate", 1, 1, 2 }, { "seau_d_eau", 1, 1, 3 }, { "lingot_de_fer", 1, 4, 4 },
}
local LOOT_ARMOR = { "chapeau_kasa", "veste_de_cuir", "hakama_de_cuir", "sandales_waraji" }
local LOOT_MID = {
  { "katana_en_fer", 1, 1, 7 }, { "katana_en_diamant", 1, 1, 2 }, { "arc_yumi", 1, 1, 6 }, { "fleche", 8, 16, 8 },
  { "feux_d_artifice", 1, 3, 4 }, { "pomme", 2, 4, 6 }, { "diamant", 1, 3, 4 }, { "obsidienne", 4, 8, 4 },
  { "naginata", 1, 1, 3 }, { "massue_tetsubo", 1, 1, 2 }, { "planches_de_hinoki", 24, 48, 8 }, { "viande_cuite", 3, 6, 7 },
}
local LOOT_MID_ARMOR = { "casque_kabuto_de_fer", "cuirasse_do_de_fer", "cuissards_haidate_de_fer", "jambieres_suneate_de_fer" }

rules.set("mobs", false)
rules.set("pvp", false)
rules.set("regen", true)

local phase = "attente"     -- attente, cage, jeu, fin
local wait, ticks, clock = COUNTDOWN, 0, 0
local ox, oz = 0, 0         -- centre de l'arene
local built = false
local island_of = {}        -- pseudo -> numero de son ile
local alive, count = {}, 0
local kills = {}
local chests = {}           -- { x, y, z, centre? }
local seed = 12345

-- Un tirage simple et reproductible : les scripts n'ont pas d'horloge a eux.
local function roll(n)
  seed = (seed * 1103515245 + 12345) % 2147483648
  return seed % n
end

local function island(i)
  local a = (i - 1) / ISLANDS * 2 * math.pi
  return ox + math.floor(math.cos(a) * RADIUS + 0.5), oz + math.floor(math.sin(a) * RADIUS + 0.5)
end

-- Un objet existe-t-il sous ce nom ? Une carte ne doit pas tomber en panne pour un butin mal nomme.
local function put(x, y, z, item, n)
  return pcall(world.chest, x, y, z, item, n)
end

local function fill_chest(c)
  local list, armor = c[4] and LOOT_MID or LOOT, c[4] and LOOT_MID_ARMOR or LOOT_ARMOR
  local placed = 0
  for _, item in ipairs(list) do
    if roll(10) < item[4] and placed < (c[4] and 9 or 7) then
      put(c[1], c[2], c[3], item[1], item[2] + roll(item[3] - item[2] + 1))
      placed = placed + 1
    end
  end
  -- Toujours de quoi batir, et une piece d'armure une fois sur deux.
  put(c[1], c[2], c[3], "planches_de_hinoki", 16)
  if roll(2) == 0 then put(c[1], c[2], c[3], armor[roll(#armor) + 1], 1) end
end

-- Une ile : un socle de roche qui s'effile vers le bas, de la terre, de l'herbe, un arbre et deux coffres.
local function build_island(ix, iz, r, mid)
  for d = 0, 4 do
    local rr = math.max(1, r - d)
    local block = d == 0 and "herbe" or d < 2 and "terre" or "pierre"
    world.fill(ix - rr, HEIGHT - d, iz - rr, ix + rr, HEIGHT - d, iz + rr, block)
  end
  if mid then
    -- Le centre : une estrade de pierre, quatre coffres et un minerai de diamant en son coeur.
    world.fill(ix - 2, HEIGHT + 1, iz - 2, ix + 2, HEIGHT + 1, iz + 2, "pierre_de_muraille")
    world.set(ix, HEIGHT + 1, iz, "minerai_de_diamant")
    for _, o in ipairs({ { 2, 0 }, { -2, 0 }, { 0, 2 }, { 0, -2 } }) do
      chests[#chests + 1] = { ix + o[1], HEIGHT + 2, iz + o[2], true }
    end
  else
    -- Un petit arbre pour le bois, et les coffres de part et d'autre.
    world.fill(ix + 2, HEIGHT + 1, iz + 2, ix + 2, HEIGHT + 3, iz + 2, "tronc_de_cedre")
    world.fill(ix + 1, HEIGHT + 4, iz + 1, ix + 3, HEIGHT + 5, iz + 3, "feuilles")
    chests[#chests + 1] = { ix - 2, HEIGHT + 1, iz, false }
    chests[#chests + 1] = { ix, HEIGHT + 1, iz - 2, false }
  end
end

local function cage(ix, iz, open)
  local y = HEIGHT + 7
  if open then
    world.fill(ix - 1, y, iz - 1, ix + 1, y + 3, iz + 1, "air")
  else
    world.fill(ix - 1, y, iz - 1, ix + 1, y + 3, iz + 1, "paroi_shoji")
    world.fill(ix, y + 1, iz, ix, y + 2, iz, "air")
  end
end

-- La carte se batit pendant l'attente, une ile a la fois : elle est prete quand la partie commence.
local todo = nil            -- iles qui restent a batir (0 : le centre)

local function plan(x, z)
  ox, oz = math.floor(x), math.floor(z)
  seed = (ox * 7919 + oz * 104729 + ticks) % 2147483648
  chests, todo, built = {}, { 0 }, false
  for i = 1, ISLANDS do todo[#todo + 1] = i end
end

local function build_step()
  if not todo then return end
  local i = table.remove(todo, 1)
  local first = #chests + 1
  if i == 0 then
    build_island(ox, oz, 7, true)
  else
    local ix, iz = island(i)
    build_island(ix, iz, 4, false)
  end
  for k = first, #chests do fill_chest(chests[k]) end
  if #todo == 0 then todo, built = nil, true end
end

local function status(p)
  if phase == "attente" then
    local n = #players()
    if n < MIN_PLAYERS then return "Sky Wars - en attente de joueurs (" .. n .. "/" .. MIN_PLAYERS .. ")" end
    return "Sky Wars - depart dans " .. wait .. " s"
  elseif phase == "cage" then
    return "Sky Wars - les cages s'ouvrent dans " .. math.max(0, CAGE - clock) .. " s"
  elseif phase == "jeu" then
    local text = "Sky Wars - " .. count .. " en vie"
    if clock < REFILL then text = text .. " - coffres regarnis dans " .. (REFILL - clock) .. " s" end
    local k = kills[p.name] or 0
    return text .. " - " .. k .. " victoire" .. (k > 1 and "s" or "")
  end
  return "Sky Wars - partie terminee"
end

local function finish()
  phase = "fin"
  rules.set("pvp", false)
  local winner
  for name in pairs(alive) do winner = name end
  local text = winner and (winner .. " remporte la partie de Sky Wars !") or "Sky Wars termine : personne n'est reste debout."
  announce(text)
  say(text)
end

local function out(name, why)
  if (phase == "jeu" or phase == "cage") and alive[name] then
    alive[name] = nil
    count = count - 1
    say(name .. " est elimine" .. why .. ". Il reste " .. count .. " joueur" .. (count > 1 and "s" or "") .. ".")
    if count <= 1 then finish() end
  end
end

local eager = false         -- depart demande avant la fin de la construction
local function start()
  local list = players()
  -- La carte n'est pas finie : la partie partira des qu'elle le sera.
  if not built then
    if not todo then
      local x, _, z = list[1]:pos()
      plan(x, z)
    end
    eager = true
    return
  end
  eager = false
  island_of, alive, count, kills = {}, {}, 0, {}
  -- Les joueurs sont repartis sur des iles aussi eloignees que possible les unes des autres.
  local step = math.max(1, math.floor(ISLANDS / math.min(#list, ISLANDS)))
  for i, p in ipairs(list) do
    if i <= ISLANDS then
      local n = ((i - 1) * step) % ISLANDS + 1
      island_of[p.name] = n
      local ix, iz = island(n)
      cage(ix, iz, false)
      p:clear()
      p:set_mode("aventure")
      p:set_health(20)
      p:set_hunger(20)
      p:teleport(ix + 0.5, HEIGHT + 8.2, iz + 0.5)
      alive[p.name] = true
      count = count + 1
    else
      p:set_mode("spectateur")
      p:tell("Les huit iles sont prises : vous regardez cette partie.")
    end
  end
  phase, clock = "cage", 0
  world.set_time(10)
  announce("Sky Wars : les cages s'ouvrent dans " .. CAGE .. " secondes !")
end

local function release()
  for name, n in pairs(island_of) do
    if alive[name] then
      local ix, iz = island(n)
      cage(ix, iz, true)
    end
  end
  for _, p in ipairs(players()) do
    if alive[p.name] then p:set_mode("survie") end
  end
  phase, clock = "jeu", 0
  rules.set("pvp", true)
  announce("C'est parti ! Videz vos coffres, le centre est mieux garni.")
end

on("join", function(p)
  if phase == "attente" then
    p:set_mode("aventure")
    p:tell("Bienvenue dans Sky Wars ! La partie commence des que " .. MIN_PLAYERS .. " joueurs sont la.")
  else
    p:set_mode("spectateur")
    p:tell("La partie est en cours : vous la regardez en spectateur.")
  end
end)

on("leave", function(p) out(p.name, " (parti)") end)

on("death", function(p, killer)
  local by = killer ~= "" and killer or nil
  if by and alive[p.name] then kills[by] = (kills[by] or 0) + 1 end
  out(p.name, by and (" par " .. by) or "")
end)

-- Pas de seconde vie : on revient en spectateur, au-dessus du centre.
on("respawn", function(p)
  if phase == "attente" then return end
  p:set_mode("spectateur")
  p:teleport(ox + 0.5, HEIGHT + 12, oz + 0.5)
end)

-- Dans la cage, on ne casse ni ne pose rien ; hors de l'arene non plus.
local function inside(x, y, z)
  return math.abs(x - ox) <= RADIUS + 12 and math.abs(z - oz) <= RADIUS + 12 and y >= HEIGHT - 12 and y <= HEIGHT + 30
end
on("break", function(p, x, y, z) if phase ~= "jeu" or not inside(x, y, z) then return false end end)
on("place", function(p, x, y, z) if phase ~= "jeu" or not inside(x, y, z) then return false end end)

on("tick", function()
  ticks = ticks + 1
  -- Des qu'un joueur est la, la carte se batit autour de lui, une ile tous les quarts de seconde.
  if phase == "attente" and ticks % 5 == 0 then
    if todo then
      build_step()
      if built and eager then start() end
    elseif not built then
      local first = players()[1]
      local x, _, z = nil, nil, nil
      if first then x, _, z = first:pos() end
      if x then plan(x, z) end
    end
  end
  if ticks % 20 ~= 0 then return end
  local list = players()
  if phase == "attente" then
    if #list >= MIN_PLAYERS then
      wait = wait - 1
      if wait <= 0 then start() elseif wait <= 5 or wait % 5 == 0 then announce("Depart dans " .. wait .. " s") end
    else
      wait = COUNTDOWN
    end
  elseif phase == "cage" then
    clock = clock + 1
    if clock >= CAGE then release() elseif CAGE - clock <= 3 then announce(tostring(CAGE - clock)) end
  elseif phase == "jeu" then
    clock = clock + 1
    if clock == REFILL then
      for _, c in ipairs(chests) do fill_chest(c) end
      announce("Les coffres sont regarnis !")
    end
    if clock == SUDDEN then announce("Mort subite : la vie ne remonte plus, et l'air se fait rare.") rules.set("regen", false) end
    for _, p in ipairs(list) do
      if alive[p.name] then
        local _, y = p:pos()
        -- Tomber dans le vide, c'est mourir. A la mort subite, tout le monde perd de la vie.
        if y and y < HEIGHT - 30 then p:damage(1000) elseif clock > SUDDEN and clock % 3 == 0 then p:damage(1) end
      end
    end
  end
  for _, p in ipairs(list) do p:send("etat", status(p)) end
end)

command("skywars", "Lance la partie sans attendre (avec « demarrer »), ou dit ou elle en est", function(p, args)
  if args == "demarrer" and phase == "attente" then
    start()
    return "Sky Wars lance."
  end
  if args == "ouvrir" and phase == "cage" then
    release()
    return "Cages ouvertes."
  end
  return status(p)
end)
