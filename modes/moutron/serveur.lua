-- MouTron (d'apres le jeu d'Alaric sur Epicube, inspire du film Tron) : chacun laisse derriere lui une
-- trainee de couleur sur le sol de l'arene. Marcher sur une trainee, la sienne ou celle d'un autre, c'est
-- perdre ; rester sur place aussi. Le dernier en course gagne. (Ici on court a pied : pas de monture.)
local COLORS = { "bloc_de_braise", "bloc_de_reiseki", "mousse_lumineuse", "glace", "fourrure", "tuiles_kawara", "mousse_pourpre", "bloc_d_ossements" }
local R = 20
local marks, last, color, idle = {}, {}, {}, {}

minigame{ id = "moutron", name = "MouTron", min = 2, pvp = false,
  build = function(g)
    marks, last, idle = {}, {}, {}
    world.fill(g.ox - R, g.oy, g.oz - R, g.ox + R, g.oy, g.oz + R, "pierre")
    for _, w in ipairs({ { -R, -R, R, -R }, { -R, R, R, R }, { -R, -R, -R, R }, { R, -R, R, R } }) do
      world.fill(g.ox + w[1], g.oy + 1, g.oz + w[2], g.ox + w[3], g.oy + 3, g.oz + w[4], "obsidienne")
    end
  end,
  spawn = function(g, p, i)
    local x, z = g:ring(i, R - 5)
    color[p.name] = COLORS[(i - 1) % #COLORS + 1]
    p:teleport(x + 0.5, g.oy + 1.2, z + 0.5)
    p:tell("Courez sans vous arreter : votre trainee et celles des autres sont mortelles.")
  end,
  tick = function(g)
    -- Trois secondes pour s'elancer avant que les trainees comptent.
    if g.ticks % 2 ~= 0 or g.clock < 3 then return end
    for _, p in ipairs(g:players()) do
      local x, _, z = p:pos()
      if x then
        local cx, cz = math.floor(x), math.floor(z)
        local key = cx .. "," .. cz
        local was = last[p.name]
        if not was or was.key ~= key then
          if marks[key] then
            p:damage(1000)
          elseif was then
            marks[was.key] = true
            world.set(was.x, g.oy, was.z, color[p.name])
          end
          last[p.name], idle[p.name] = { key = key, x = cx, z = cz }, 0
        else
          idle[p.name] = (idle[p.name] or 0) + 1
          if idle[p.name] > 15 then p:damage(1000) end
        end
      end
    end
  end,
  status = function(g) return g.count .. " en course" end }
