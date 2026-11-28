-- SkyFall : trois etages de neige au-dessus du vide. Chaque bloc ou l'on pose le pied s'efface un instant
-- plus tard : on tombe d'etage en etage, puis dans le vide. Le dernier a ne pas tomber gagne.
local R, GAP = 9, 7
local doomed = {}

minigame{ id = "skyfall", name = "SkyFall", min = 2, void = 3 * 7 + 6, pvp = false,
  build = function(g)
    doomed = {}
    for floor = 0, 2 do
      world.fill(g.ox - R, g.oy - floor * GAP, g.oz - R, g.ox + R, g.oy - floor * GAP, g.oz + R, "neige")
    end
  end,
  spawn = function(g, p, i)
    local x, z = g:ring(i, 6)
    p:teleport(x + 0.5, g.oy + 1.2, z + 0.5)
  end,
  tick = function(g)
    if g.clock < 3 then return end
    -- Les blocs condamnes il y a un instant disparaissent ; ceux que l'on foule le sont a leur tour.
    local now = g.ticks
    for key, d in pairs(doomed) do
      if now >= d.at then
        world.set(d.x, d.y, d.z, "air")
        doomed[key] = nil
      end
    end
    for _, p in ipairs(g:players()) do
      local x, y, z = p:pos()
      if x then
        local bx, by, bz = math.floor(x), math.floor(y - 0.5), math.floor(z)
        local key = bx .. "," .. by .. "," .. bz
        if not doomed[key] and (g.oy - by) % GAP == 0 and by <= g.oy and by >= g.oy - 2 * GAP then
          doomed[key] = { x = bx, y = by, z = bz, at = now + 8 }
        end
      end
    end
  end,
  status = function(g) return g.count .. " encore en l'air" end }
