-- Fallen Kingdoms (concept de Zelvac, revu par Trifcan pour Epicube : « vous devez proteger le coeur de votre
-- base contre les assauts ennemis ») : jusqu'a quatre royaumes dans un monde de survie. On recolte, on
-- batit, on s'equipe ; le combat s'ouvre apres deux minutes, les assauts apres cinq. Tant que son coeur bat,
-- une equipe revient en jeu apres chaque mort. Le dernier royaume debout gagne.
local PEACE = 120          -- secondes sans combat entre joueurs
local ASSAULT = 300        -- secondes avant que les coeurs puissent etre attaques
local HEART = 5            -- coups que tient un coeur
blocks.register{ id = "coeur", name = "Coeur du royaume", hardness = 2, light = 12, drop = false }
local CORE = "fallenkingdoms:coeur"
local hp, at = {}, {}      -- par equipe : vie du coeur, place de la base
local AGES = {}

local function kit(p)
  p:give("katana_en_pierre", 1)
  p:give("pioche_en_pierre", 1)
  p:give("hache_ono_en_pierre", 1)
  p:give("onigiri", 6)
end

local the_game
the_game = minigame{ id = "fallenkingdoms", name = "Fallen Kingdoms", min = 2, teams = { "Rouge", "Bleue", "Verte", "Jaune" }, mode = "survie", height = 0,
  build = function(g)
    hp, at = {}, {}
    local dirs = { { 1, 0 }, { -1, 0 }, { 0, 1 }, { 0, -1 } }
    for team = 1, math.min(4, g.total) do
      -- Chaque royaume est bati sur le sol, a soixante blocs du centre : une esplanade, et son coeur.
      local bx, bz = g.ox + dirs[team][1] * 60, g.oz + dirs[team][2] * 60
      local by = (world.top(bx, bz) or 100) + 1
      world.fill(bx - 5, by, bz - 5, bx + 5, by, bz + 5, "pierre_de_muraille")
      world.fill(bx - 5, by + 1, bz - 5, bx + 5, by + 5, bz + 5, "air")
      world.set(bx, by + 1, bz, CORE)
      for _, o in ipairs({ { -5, -5 }, { 5, 5 }, { -5, 5 }, { 5, -5 } }) do
        world.fill(bx + o[1], by + 1, bz + o[2], bx + o[1], by + 3, bz + o[2], "bois_laque_vermillon")
      end
      hp[team], at[team] = HEART, { bx, by, bz }
    end
  end,
  spawn = function(g, p, i, team)
    local b = at[team]
    p:set_home(b[1] + 2.5, b[2] + 2, b[3] + 0.5)
    p:teleport(b[1] + 2.5, b[2] + 1.2, b[3] + 0.5)
    kit(p)
  end,
  start = function(g)
    rules.set("pvp", false)
    world.set_time(7)
    say("Batissez et equipez-vous : le combat s'ouvre dans " .. PEACE .. " secondes, les assauts dans " .. ASSAULT .. ".")
  end,
  -- Tant que le coeur de son royaume bat, on y revient apres chaque mort.
  death = function(g, p) return (hp[g:team(p)] or 0) > 0 end,
  respawn = function(g, p) kit(p) end,
  second = function(g, clock)
    if clock == PEACE then
      rules.set("pvp", true)
      announce("Le combat entre royaumes est ouvert !")
    end
    if clock == ASSAULT then announce("Les assauts sont ouverts : allez detruire les coeurs !") end
    for _, age in ipairs(AGES) do
      if clock == age[1] then
        announce(age[2])
        for _, p in ipairs(g:players()) do for _, item in ipairs(age[3]) do p:give(item, 1) end end
      end
    end
  end,
  status = function(g, p)
    local parts = {}
    for team = 1, 4 do if hp[team] then parts[#parts + 1] = g:team_name(team) .. " " .. (hp[team] > 0 and (hp[team] .. " coeur") or "tombe") end end
    local soon = g.clock < PEACE and (" - combat dans " .. (PEACE - g.clock) .. " s") or g.clock < ASSAULT and (" - assauts dans " .. (ASSAULT - g.clock) .. " s") or ""
    return table.concat(parts, " | ") .. soon
  end }

-- Le coeur d'un royaume ne se casse pas d'un coup : chaque coup porte lui retire un point de vie.
on("break", function(p, x, y, z, block)
  local g = the_game
  if block ~= CORE then return end
  if g.phase ~= "jeu" then return false end
  local whose
  for team, b in pairs(at) do if b[1] == x and b[3] == z then whose = team end end
  if not whose or g:team(p) == whose then
    p:info("C'est le coeur de votre royaume !")
    return false
  end
  if g.clock < ASSAULT then
    p:info("Les assauts ne sont pas encore ouverts.")
    return false
  end
  hp[whose] = hp[whose] - 1
  world.sound("explosion", x, y, z, 0.7)
  if hp[whose] > 0 then
    announce("Le coeur de l'equipe " .. g:team_name(whose) .. " est attaque ! (" .. hp[whose] .. " / " .. HEART .. ")")
    return false
  end
  announce("Le coeur de l'equipe " .. g:team_name(whose) .. " est detruit par " .. p.name .. " : plus de retour en jeu pour elle !")
  say("Le royaume de l'equipe " .. g:team_name(whose) .. " est tombe.")
  return true
end)
