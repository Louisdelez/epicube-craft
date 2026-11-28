-- King Of The Hill : une colline au centre de l'arene. Qui se tient seul a son sommet marque un point
-- par seconde ; a plusieurs, personne ne marque. Le premier a 60 points est roi. On revient en jeu apres chaque mort.
local GOAL = 60
local score = {}

local function kit(p)
  p:give("katana_en_pierre", 1)
  p:give("cuirasse_do_de_bambou", 1)
  p:give("onigiri", 4)
end

minigame{ id = "kingofthehill", name = "King Of The Hill", min = 2, void = 14, endless = true,
  build = function(g)
    world.fill(g.ox - 14, g.oy, g.oz - 14, g.ox + 14, g.oy, g.oz + 14, "herbe")
    world.fill(g.ox - 5, g.oy + 1, g.oz - 5, g.ox + 5, g.oy + 1, g.oz + 5, "pierre")
    world.fill(g.ox - 3, g.oy + 2, g.oz - 3, g.ox + 3, g.oy + 2, g.oz + 3, "pierre")
    world.fill(g.ox - 1, g.oy + 3, g.oz - 1, g.ox + 1, g.oy + 3, g.oz + 1, "tuiles_kawara")
  end,
  spawn = function(g, p, i)
    local x, z = g:ring(i, 12)
    score[p.name] = 0
    p:set_home(x + 0.5, g.oy + 2, z + 0.5)
    p:teleport(x + 0.5, g.oy + 1.2, z + 0.5)
    kit(p)
  end,
  death = function() return true end,
  respawn = function(g, p) kit(p) end,
  second = function(g)
    local on, king = 0, nil
    for _, p in ipairs(g:players()) do
      local x, y, z = p:pos()
      if x and math.abs(x - g.ox - 0.5) <= 1.6 and math.abs(z - g.oz - 0.5) <= 1.6 and y >= g.oy + 3.5 then on, king = on + 1, p end
    end
    if on == 1 then
      score[king.name] = (score[king.name] or 0) + 1
      if score[king.name] >= GOAL then g:win(king.name .. " est le roi de la colline !") end
    end
  end,
  status = function(g, p)
    local best, who = 0, "personne"
    for name, s in pairs(score) do if s > best then best, who = s, name end end
    return "vous " .. (score[p.name] or 0) .. " / " .. GOAL .. " - en tete : " .. who .. " " .. best
  end }
