-- PitchOut (d'apres le jeu de Yoga_Sama sur Epicube) : tout le monde sur une plateforme dans le ciel,
-- cinq vies chacun et un baton au recul enorme. Qui tombe perd une vie ; le dernier en jeu gagne.
local LIVES = 5
local lives = {}

local function kit(p)
  p:give_enchanted("bokken", "recul", 5)
end

minigame{ id = "pitchout", name = "PitchOut", min = 2, void = 14,
  build = function(g)
    world.fill(g.ox - 9, g.oy, g.oz - 9, g.ox + 9, g.oy, g.oz + 9, "pierre_de_muraille")
    world.fill(g.ox - 2, g.oy + 1, g.oz - 2, g.ox + 2, g.oy + 1, g.oz + 2, "tuiles_kawara")
  end,
  spawn = function(g, p, i)
    local x, z = g:ring(i, 6)
    lives[p.name] = LIVES
    p:set_home(g.ox + 0.5, g.oy + 3, g.oz + 0.5)
    p:teleport(x + 0.5, g.oy + 1.2, z + 0.5)
    kit(p)
  end,
  death = function(g, p)
    lives[p.name] = (lives[p.name] or 1) - 1
    if lives[p.name] > 0 then
      p:info("Il vous reste " .. lives[p.name] .. " vie" .. (lives[p.name] > 1 and "s" or "") .. ".")
      return true
    end
  end,
  respawn = function(g, p)
    p:teleport(g.ox + 0.5, g.oy + 3, g.oz + 0.5)
    kit(p)
  end,
  status = function(g, p)
    return g.count .. " en jeu - " .. (lives[p.name] or 0) .. " vie" .. ((lives[p.name] or 0) > 1 and "s" or "")
  end }
