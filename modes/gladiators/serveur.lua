-- Gladiators : deux equipes face a face dans une arene fermee. Une manche se joue sans retour en jeu :
-- l'equipe qui reste debout la remporte. Tout le monde revient a la manche suivante ; la premiere equipe
-- a trois manches gagne.
local GOAL = 3
local wins, down, round, pause = { 0, 0 }, {}, 1, 0

local function kit(p)
  p:clear()
  p:give("katana_en_fer", 1)
  p:give("bouclier_tate", 1)
  p:give("casque_kabuto_de_fer", 1)
  p:give("cuirasse_do_de_fer", 1)
  p:give("viande_cuite", 2)
end

local function place(g, p, k)
  local side = g:team(p) == 1 and -1 or 1
  p:set_mode("aventure")
  p:set_health(20)
  p:teleport(g.ox + side * 9 + 0.5, g.oy + 1.2, g.oz + ((k * 3) % 13) - 6 + 0.5)
  kit(p)
end

local function standing(g, team)
  local n = 0
  for _, p in ipairs(g:players()) do if g:team(p) == team and not down[p.name] then n = n + 1 end end
  return n
end

minigame{ id = "gladiators", name = "Gladiators", min = 2, teams = { "Rouge", "Bleue" }, endless = true,
  build = function(g)
    wins, down, round, pause = { 0, 0 }, {}, 1, 0
    world.fill(g.ox - 12, g.oy, g.oz - 9, g.ox + 12, g.oy, g.oz + 9, "sable")
    for _, w in ipairs({ { -12, -9, 12, -9 }, { -12, 9, 12, 9 }, { -12, -9, -12, 9 }, { 12, -9, 12, 9 } }) do
      world.fill(g.ox + w[1], g.oy + 1, g.oz + w[2], g.ox + w[3], g.oy + 5, g.oz + w[4], "pierre_de_muraille")
    end
  end,
  spawn = function(g, p, i)
    p:set_home(g.ox + 0.5, g.oy + 8, g.oz + 0.5)
    place(g, p, i)
  end,
  death = function(g, p)
    down[p.name] = true
    return true
  end,
  -- Tombe pendant la manche : on la regarde d'en haut jusqu'a la suivante.
  respawn = function(g, p)
    p:set_mode("spectateur")
    p:teleport(g.ox + 0.5, g.oy + 8, g.oz + 0.5)
  end,
  second = function(g)
    if pause > 0 then
      pause = pause - 1
      if pause == 0 then
        down, round = {}, round + 1
        for k, p in ipairs(g:players()) do place(g, p, k) end
        announce("Manche " .. round .. " !")
      end
      return
    end
    local a, b = standing(g, 1), standing(g, 2)
    if a > 0 and b > 0 then return end
    local won = a > 0 and 1 or 2
    wins[won] = wins[won] + 1
    if wins[won] >= GOAL then
      g:win("L'equipe " .. g:team_name(won) .. " remporte les Gladiators " .. wins[1] .. " a " .. wins[2] .. " !")
    else
      announce("Manche pour l'equipe " .. g:team_name(won) .. " (" .. wins[1] .. " - " .. wins[2] .. ")")
      pause = 4
    end
  end,
  status = function(g, p) return "manche " .. round .. " - Rouge " .. wins[1] .. " / Bleue " .. wins[2] end }
