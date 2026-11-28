-- EpicSmash : des plateformes au-dessus du vide, trois vies chacun. Les coups repoussent, et de plus en
-- plus fort a mesure que la partie dure. Qui tombe perd une vie ; le dernier en jeu gagne.
local LIVES = 3
local lives = {}

local function power(g) return math.min(10, 2 + math.floor(g.clock / 30)) end

local function kit(g, p)
  p:clear()
  p:give_enchanted("bokken", "recul", power(g))
end

minigame{ id = "epicsmash", name = "EpicSmash", min = 2, void = 16,
  build = function(g)
    world.fill(g.ox - 7, g.oy, g.oz - 4, g.ox + 7, g.oy, g.oz + 4, "pierre_de_muraille")
    world.fill(g.ox - 13, g.oy + 3, g.oz - 2, g.ox - 10, g.oy + 3, g.oz + 2, "planches_de_hinoki")
    world.fill(g.ox + 10, g.oy + 3, g.oz - 2, g.ox + 13, g.oy + 3, g.oz + 2, "planches_de_hinoki")
    world.fill(g.ox - 2, g.oy + 5, g.oz - 2, g.ox + 2, g.oy + 5, g.oz + 2, "planches_de_hinoki")
  end,
  spawn = function(g, p, i)
    lives[p.name] = LIVES
    p:set_home(g.ox + 0.5, g.oy + 7, g.oz + 0.5)
    p:teleport(g.ox - 6 + ((i - 1) * 3) % 13 + 0.5, g.oy + 1.2, g.oz + 0.5)
    kit(g, p)
  end,
  death = function(g, p)
    lives[p.name] = (lives[p.name] or 1) - 1
    if lives[p.name] > 0 then return true end
  end,
  respawn = function(g, p) kit(g, p) end,
  second = function(g, clock)
    if clock % 30 == 0 and power(g) <= 10 then
      announce("Les coups portent plus loin !")
      for _, p in ipairs(g:players()) do kit(g, p) end
    end
  end,
  status = function(g, p) return g.count .. " en jeu - " .. (lives[p.name] or 0) .. " vie(s) - force " .. power(g) end }
