-- FightClub : une arene fermee, le meme equipement pour tout le monde, et l'on revient aussitot apres
-- chaque mort. Le premier a 15 victoires l'emporte.
local GOAL = 15

local function kit(p)
  p:give("katana_en_fer", 1)
  p:give("arc_yumi", 1)
  p:give("fleche", 12)
  p:give("casque_kabuto_de_fer", 1)
  p:give("cuirasse_do_de_fer", 1)
  p:give("viande_cuite", 4)
end

minigame{ id = "fightclub", name = "FightClub", min = 2, endless = true,
  build = function(g)
    world.fill(g.ox - 13, g.oy, g.oz - 13, g.ox + 13, g.oy, g.oz + 13, "sable")
    for _, w in ipairs({ { -13, -13, 13, -13 }, { -13, 13, 13, 13 }, { -13, -13, -13, 13 }, { 13, -13, 13, 13 } }) do
      world.fill(g.ox + w[1], g.oy + 1, g.oz + w[2], g.ox + w[3], g.oy + 4, g.oz + w[4], "pierre_de_muraille")
    end
    for _, o in ipairs({ { -5, -5 }, { 5, 5 }, { -5, 5 }, { 5, -5 } }) do
      world.fill(g.ox + o[1], g.oy + 1, g.oz + o[2], g.ox + o[1], g.oy + 2, g.oz + o[2], "bois_laque_vermillon")
    end
  end,
  spawn = function(g, p, i)
    local x, z = g:ring(i, 10)
    p:set_home(x + 0.5, g.oy + 2, z + 0.5)
    p:teleport(x + 0.5, g.oy + 1.2, z + 0.5)
    kit(p)
  end,
  death = function(g, p, by)
    if by and (g.kills[by] or 0) >= GOAL then g:win(by .. " remporte le FightClub avec " .. GOAL .. " victoires !") end
    return true
  end,
  respawn = function(g, p) kit(p) end,
  status = function(g, p)
    local best, who = 0, "personne"
    for name, k in pairs(g.kills) do if k > best then best, who = k, name end end
    return "vous " .. (g.kills[p.name] or 0) .. " / " .. GOAL .. " - en tete : " .. who .. " " .. best
  end }
