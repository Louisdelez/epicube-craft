-- Hunger Games : les tributs attendent sur leurs socles, en cercle autour de la corne d'abondance et
-- de ses coffres. Au signal, on court s'y armer ou l'on fuit vers les coffres disperses dans l'arene.
-- On ne casse ni ne pose rien : on vit de ce que l'on trouve. Les coffres se regarnissent une fois ;
-- puis vient le match a mort : les survivants sont ramenes a la corne, dans une zone qui se referme.
-- Le dernier tribut en vie gagne.

local MIN_PLAYERS = 2
local COUNTDOWN = 20        -- secondes d'attente une fois assez de joueurs reunis
local HOLD = 10             -- secondes passees sur le socle avant le signal
local GRACE = 15            -- secondes sans combat apres le signal : le temps de courir
local TRIBUTES = 12         -- socles autour de la corne
local RING = 11             -- du centre a chaque socle
local ARENA = 90            -- demi-cote de l'arene, en blocs
local CACHES = 24           -- coffres disperses dans l'arene
local REFILL = 300          -- secondes avant que les coffres se regarnissent
local DEATHMATCH = 600      -- secondes avant le match a mort
local FINAL = 18            -- demi-cote de la zone du match a mort

-- Butin : objet, minimum, maximum, chances sur 10. La corne est mieux garnie que les coffres de l'arene.
local LOOT = {
  { "bokken", 1, 1, 6 }, { "katana_en_pierre", 1, 1, 3 }, { "hache_ono_en_bois", 1, 1, 4 }, { "dague_tanto", 1, 1, 3 },
  { "onigiri", 1, 3, 8 }, { "pomme", 1, 2, 6 }, { "viande_crue", 1, 2, 5 }, { "daikon", 1, 3, 5 },
  { "fleche", 2, 5, 4 }, { "kunai", 1, 3, 4 }, { "baton", 1, 3, 5 }, { "cuir", 1, 2, 4 },
  { "chapeau_kasa", 1, 1, 3 }, { "veste_de_cuir", 1, 1, 3 }, { "hakama_de_cuir", 1, 1, 3 }, { "sandales_waraji", 1, 1, 3 },
}
local LOOT_CORN = {
  { "katana_en_fer", 1, 1, 5 }, { "katana_en_pierre", 1, 1, 8 }, { "arc_yumi", 1, 1, 6 }, { "fleche", 6, 12, 8 },
  { "lance_yari", 1, 1, 3 }, { "naginata", 1, 1, 2 }, { "bouclier_tate", 1, 1, 4 }, { "kunai", 3, 6, 5 },
  { "viande_cuite", 2, 4, 7 }, { "onigiri", 3, 6, 8 }, { "pomme", 2, 4, 5 },
  { "casque_kabuto_de_fer", 1, 1, 3 }, { "cuirasse_do_de_fer", 1, 1, 3 }, { "cuissards_haidate_de_fer", 1, 1, 3 }, { "jambieres_suneate_de_fer", 1, 1, 3 },
  { "cuirasse_do_de_bambou", 1, 1, 5 }, { "casque_kabuto_de_bambou", 1, 1, 5 },
}

rules.set("mobs", false)
rules.set("pvp", false)
rules.set("regen", true)

local phase = "attente"     -- attente, socle, jeu, fin
local wait, ticks, clock = COUNTDOWN, 0, 0
local ox, oy, oz = 0, 0, 0  -- centre de l'arene, au sol
local built, todo = false, nil
local chests = {}           -- { x, y, z, corne? }
local post = {}             -- pseudo -> { x, y, z } : son socle
local alive, count, started_with = {}, 0, 0
local kills = {}
local final = false
local seed = 4242

local function roll(n)
  seed = (seed * 1103515245 + 12345) % 2147483648
  return seed % n
end

-- Une carte ne doit pas tomber en panne pour un butin mal nomme.
local function put(c, item, n) return pcall(world.chest, c[1], c[2], c[3], item, n) end

local function fill_chest(c)
  local list, most = c[4] and LOOT_CORN or LOOT, c[4] and 6 or 4
  local placed = 0
  for _, item in ipairs(list) do
    if placed < most and roll(10) < item[4] then
      put(c, item[1], item[2] + roll(item[3] - item[2] + 1))
      placed = placed + 1
    end
  end
  if placed == 0 then put(c, "onigiri", 2) end
end

local function pedestal(i)
  local a = (i - 1) / TRIBUTES * 2 * math.pi
  return ox + math.floor(math.cos(a) * RING + 0.5), oz + math.floor(math.sin(a) * RING + 0.5)
end

-- La corne d'abondance : une esplanade degagee, un pavillon au centre et ses huit coffres, les socles autour.
local function build_center()
  world.fill(ox - 14, oy, oz - 14, ox + 14, oy, oz + 14, "pierre_de_muraille")
  world.fill(ox - 14, oy + 1, oz - 14, ox + 14, oy + 4, oz + 14, "air")
  world.fill(ox - 14, oy + 5, oz - 14, ox + 14, oy + 9, oz + 14, "air")
  world.fill(ox - 3, oy + 1, oz - 3, ox + 3, oy + 1, oz + 3, "tuiles_kawara")
  for _, o in ipairs({ { -3, -3 }, { 3, -3 }, { -3, 3 }, { 3, 3 } }) do
    world.fill(ox + o[1], oy + 2, oz + o[2], ox + o[1], oy + 4, oz + o[2], "bois_laque_vermillon")
  end
  world.fill(ox - 4, oy + 5, oz - 4, ox + 4, oy + 5, oz + 4, "tuiles_kawara")
  world.fill(ox - 2, oy + 6, oz - 2, ox + 2, oy + 6, oz + 2, "tuiles_kawara")
  for _, o in ipairs({ { -2, 0 }, { 2, 0 }, { 0, -2 }, { 0, 2 }, { -2, -2 }, { 2, 2 }, { -2, 2 }, { 2, -2 } }) do
    chests[#chests + 1] = { ox + o[1], oy + 2, oz + o[2], true }
  end
  for i = 1, TRIBUTES do
    local px, pz = pedestal(i)
    world.set(px, oy + 1, pz, "obsidienne")
  end
end

-- Un coffre de l'arene, pose au sol a un endroit tire au sort.
local function build_cache()
  local x, z = ox + roll(2 * ARENA - 40) - (ARENA - 20), oz + roll(2 * ARENA - 40) - (ARENA - 20)
  if math.abs(x - ox) < 20 and math.abs(z - oz) < 20 then x = x + 30 end
  local y = world.top(x, z)
  if y then chests[#chests + 1] = { x, y + 1, z, false } end
end

local function plan(x, z)
  ox, oz = math.floor(x), math.floor(z)
  oy = world.top(ox, oz) or 150
  seed = (ox * 7919 + oz * 104729 + ticks) % 2147483648
  chests, todo, built = {}, CACHES + 1, false
end

-- La carte se batit pendant l'attente, un morceau a la fois : la corne, puis chaque coffre.
local function build_step()
  if not todo then return end
  local first = #chests + 1
  if todo == CACHES + 1 then build_center() else build_cache() end
  for k = first, #chests do fill_chest(chests[k]) end
  todo = todo - 1
  if todo == 0 then todo, built = nil, true end
end

local function zone()
  return final and FINAL or ARENA
end

local function status(p)
  if phase == "attente" then
    local n = #players()
    if n < MIN_PLAYERS then return "Hunger Games - en attente de tributs (" .. n .. "/" .. MIN_PLAYERS .. ")" end
    return "Hunger Games - depart dans " .. wait .. " s"
  elseif phase == "socle" then
    return "Hunger Games - restez sur votre socle : " .. math.max(0, HOLD - clock) .. " s"
  elseif phase == "jeu" then
    local text = "Hunger Games - " .. count .. " tribut" .. (count > 1 and "s" or "") .. " en vie"
    if clock < GRACE then text = text .. " - combat dans " .. (GRACE - clock) .. " s"
    elseif final then text = text .. " - match a mort"
    elseif clock < REFILL then text = text .. " - coffres regarnis dans " .. (REFILL - clock) .. " s"
    else text = text .. " - match a mort dans " .. math.max(0, DEATHMATCH - clock) .. " s" end
    local k = kills[p.name] or 0
    return text .. " - " .. k .. " victoire" .. (k > 1 and "s" or "")
  end
  return "Hunger Games - partie terminee"
end

local function finish()
  phase = "fin"
  rules.set("pvp", false)
  local winner
  for name in pairs(alive) do winner = name end
  local text = winner and (winner .. " remporte les Hunger Games !") or "Hunger Games termines : aucun tribut n'a survecu."
  announce(text)
  say(text)
end

local function deathmatch()
  if final then return end
  final = true
  local i = 0
  for _, p in ipairs(players()) do
    if alive[p.name] then
      i = i + 1
      local px, pz = pedestal((i - 1) * math.max(1, math.floor(TRIBUTES / count)) + 1)
      p:teleport(px + 0.5, oy + 2.2, pz + 0.5)
    end
  end
  announce("Match a mort : les tributs sont ramenes a la corne d'abondance !")
  say("Match a mort ! La zone ne fait plus que " .. FINAL .. " blocs autour de la corne.")
end

local function out(name, why)
  if (phase == "jeu" or phase == "socle") and alive[name] then
    alive[name] = nil
    count = count - 1
    -- Le canon : tout le monde sait qu'un tribut est tombe.
    world.sound("explosion", ox, oy + 20, oz, 1)
    say(name .. " est tombe" .. why .. ". Il reste " .. count .. " tribut" .. (count > 1 and "s" or "") .. ".")
    if count <= 1 then finish() elseif count <= 3 and started_with > 3 and phase == "jeu" then deathmatch() end
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
  post, alive, count, kills, final = {}, {}, 0, {}, false
  for i, p in ipairs(list) do
    if i <= TRIBUTES then
      local px, pz = pedestal(i)
      post[p.name] = { px + 0.5, oy + 2.2, pz + 0.5 }
      p:clear()
      p:set_mode("aventure")
      p:set_health(20)
      p:set_hunger(20)
      p:teleport(px + 0.5, oy + 2.2, pz + 0.5)
      alive[p.name] = true
      count = count + 1
    else
      p:set_mode("spectateur")
      p:tell("Les douze socles sont pris : vous regardez cette partie.")
    end
  end
  started_with = count
  phase, clock = "socle", 0
  world.set_time(8)
  announce("Hunger Games : restez sur votre socle, le signal est dans " .. HOLD .. " secondes.")
end

on("join", function(p)
  if phase == "attente" then
    p:set_mode("aventure")
    p:tell("Bienvenue aux Hunger Games ! La partie commence des que " .. MIN_PLAYERS .. " tributs sont la.")
  else
    p:set_mode("spectateur")
    p:tell("La partie est en cours : vous la regardez en spectateur.")
  end
end)

on("leave", function(p) out(p.name, " (parti)") end)

on("death", function(p, killer)
  local by = killer ~= "" and killer or nil
  if by and alive[p.name] then kills[by] = (kills[by] or 0) + 1 end
  out(p.name, by and (", vaincu par " .. by) or "")
end)

-- Pas de seconde vie : on revient en spectateur, au-dessus de la corne.
on("respawn", function(p)
  if phase == "attente" then return end
  p:set_mode("spectateur")
  p:teleport(ox + 0.5, oy + 14, oz + 0.5)
end)

-- L'arene ne se casse pas et l'on n'y batit rien : on vit de ce que l'on trouve.
on("break", function() return false end)
on("place", function() return false end)

on("tick", function()
  ticks = ticks + 1
  if phase == "attente" and ticks % 5 == 0 then
    if todo then
      build_step()
      if built and eager then start() end
    elseif not built then
      local first = players()[1]
      local x, z
      if first then x, _, z = first:pos() end
      if x then plan(x, z) end
    end
  end
  -- Sur le socle, on ne bouge pas : qui s'en ecarte y est remis.
  if phase == "socle" then
    for _, p in ipairs(players()) do
      local at = post[p.name]
      local x, _, z = p:pos()
      if at and x and (math.abs(x - at[1]) > 0.6 or math.abs(z - at[3]) > 0.6) then p:teleport(at[1], at[2], at[3]) end
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
  elseif phase == "socle" then
    clock = clock + 1
    if clock >= HOLD then
      phase, clock = "jeu", 0
      announce("Que les Hunger Games commencent ! Combat dans " .. GRACE .. " secondes.")
    elseif HOLD - clock <= 5 then
      announce(tostring(HOLD - clock))
    end
  elseif phase == "jeu" then
    clock = clock + 1
    if clock == GRACE then
      rules.set("pvp", true)
      announce("Le combat est ouvert !")
    end
    if clock == REFILL then
      for _, c in ipairs(chests) do fill_chest(c) end
      announce("Les coffres de l'arene sont regarnis !")
    end
    if clock == DEATHMATCH - 30 then announce("Match a mort dans 30 secondes.") end
    if clock == DEATHMATCH then deathmatch() end
    -- Hors de l'arene, ou hors de la zone du match a mort, on perd de la vie.
    local r = zone()
    for _, p in ipairs(list) do
      if alive[p.name] then
        local x, _, z = p:pos()
        if x and (math.abs(x - ox) > r or math.abs(z - oz) > r) then
          p:damage(2)
          p:info("Hors de l'arene ! Revenez vers la corne d'abondance.")
        end
      end
    end
  end
  for _, p in ipairs(list) do p:send("etat", status(p)) end
end)

command("hungergames", "Lance la partie sans attendre (avec « demarrer »), ou dit ou elle en est", function(p, args)
  if args == "demarrer" and phase == "attente" then
    start()
    return "Hunger Games lances."
  end
  if args == "signal" and phase == "socle" then
    clock = HOLD
    return "Signal donne."
  end
  if args == "final" and phase == "jeu" then
    deathmatch()
    return "Match a mort."
  end
  return status(p)
end)
