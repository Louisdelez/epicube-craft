-- IceRunner (d'apres Epicube : « creez-vous un passage vers la plateforme centrale, que votre equipe devra
-- tenir jusqu'a la fin de son compteur ») : deux iles de part et d'autre d'un plateau. On batit un pont de
-- glace, on prend le plateau : tant qu'une equipe y est la plus nombreuse, son compteur descend. A zero,
-- elle gagne. On revient en jeu apres chaque mort. (Ici la glace se pose a la main : pas de boules de neige.)
local HOLD = 60
local left = { HOLD, HOLD }

local function kit(p)
  p:give("glace", 48)
  p:give("katana_en_pierre", 1)
  p:give("onigiri", 3)
end

local function base(g, team) return g.ox + (team == 1 and -22 or 22), g.oz end

minigame{ id = "icerunner", name = "IceRunner", min = 2, teams = { "Rouge", "Bleue" }, void = 18, endless = true, mode = "survie",
  build = function(g)
    left = { HOLD, HOLD }
    world.island(g.ox, g.oy, g.oz, 4, "tuiles_kawara")
    for team = 1, 2 do
      local bx, bz = base(g, team)
      world.island(bx, g.oy, bz, 4, "neige")
    end
  end,
  spawn = function(g, p, i, team)
    local bx, bz = base(g, team)
    p:set_home(bx + 0.5, g.oy + 2, bz + 0.5)
    p:teleport(bx + 0.5, g.oy + 1.2, bz + ((i * 2) % 5) - 2 + 0.5)
    kit(p)
  end,
  death = function() return true end,
  respawn = function(g, p) kit(p) end,
  second = function(g)
    local on = { 0, 0 }
    for _, p in ipairs(g:players()) do
      local x, y, z = p:pos()
      if x and math.abs(x - g.ox - 0.5) <= 4.5 and math.abs(z - g.oz - 0.5) <= 4.5 and y >= g.oy + 0.5 then
        on[g:team(p)] = on[g:team(p)] + 1
      end
    end
    local holder = on[1] > on[2] and 1 or on[2] > on[1] and 2 or nil
    if holder then
      left[holder] = left[holder] - 1
      if left[holder] <= 0 then g:win("L'equipe " .. g:team_name(holder) .. " a tenu le plateau : elle remporte IceRunner !") end
    end
  end,
  status = function(g) return "Rouge " .. left[1] .. " s - Bleue " .. left[2] .. " s" end }

-- On ne pose et ne casse que de la glace.
on("place", function(p, x, y, z, block) if block ~= "glace" then return false end end)
on("break", function(p, x, y, z, block) if block ~= "glace" then return false end end)
