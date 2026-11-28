-- Survivor (d'apres le jeu d'Unreal852 sur Epicube, readaptation du mode Zombie de Black Ops 2) : tous
-- ensemble dans une arene, contre des manches d'oni de plus en plus nombreuses. Qui tombe revient a la
-- manche suivante, si quelqu'un a tenu jusque-la. Dix manches, et c'est gagne ; tout le monde a terre, c'est perdu.
local WAVES = 10
local wave, remaining, pause, down = 0, 0, 5, {}

local function kit(p)
  p:give("katana_en_pierre", 1)
  p:give("arc_yumi", 1)
  p:give("fleche", 16)
  p:give("onigiri", 4)
end

local function send_wave(g)
  wave = wave + 1
  local n = 3 + wave * 2
  remaining = n
  for k = 1, n do
    local a = k / n * 2 * math.pi
    world.spawn("oni", g.ox + math.cos(a) * 11 + 0.5, g.oy + 1.5, g.oz + math.sin(a) * 11 + 0.5)
  end
  announce("Manche " .. wave .. " : " .. n .. " oni !")
  -- Ceux qui etaient a terre reviennent, et tout le monde recoit de quoi tenir.
  for _, p in ipairs(g:players()) do
    if down[p.name] then
      down[p.name] = nil
      p:set_mode("aventure")
      p:set_health(20)
      p:teleport(g.ox + 0.5, g.oy + 1.2, g.oz + 0.5)
      kit(p)
    end
    p:give("fleche", 6)
    p:give("onigiri", 1)
    if wave == 3 then p:give("katana_en_fer", 1) end
    if wave == 5 then p:give("cuirasse_do_de_fer", 1) end
    if wave == 7 then p:give("casque_kabuto_de_fer", 1) end
  end
end

local the_game
the_game = minigame{ id = "survivor", name = "Survivor", min = 1, pvp = false, endless = true,
  build = function(g)
    wave, remaining, pause, down = 0, 0, 5, {}
    world.fill(g.ox - 14, g.oy, g.oz - 14, g.ox + 14, g.oy, g.oz + 14, "pierre_de_muraille")
    for _, w in ipairs({ { -14, -14, 14, -14 }, { -14, 14, 14, 14 }, { -14, -14, -14, 14 }, { 14, -14, 14, 14 } }) do
      world.fill(g.ox + w[1], g.oy + 1, g.oz + w[2], g.ox + w[3], g.oy + 5, g.oz + w[4], "obsidienne")
    end
    world.fill(g.ox - 1, g.oy + 1, g.oz - 1, g.ox + 1, g.oy + 1, g.oz + 1, "tuiles_kawara")
  end,
  spawn = function(g, p, i)
    local x, z = g:ring(i, 3)
    p:set_home(g.ox + 0.5, g.oy + 8, g.oz + 0.5)
    p:teleport(x + 0.5, g.oy + 1.2, z + 0.5)
    kit(p)
  end,
  start = function(g) world.set_time(23) end,
  death = function(g, p)
    down[p.name] = true
    return true
  end,
  respawn = function(g, p)
    p:set_mode("spectateur")
    p:teleport(g.ox + 0.5, g.oy + 8, g.oz + 0.5)
  end,
  second = function(g)
    local up = 0
    for _, p in ipairs(g:players()) do if not down[p.name] then up = up + 1 end end
    if up == 0 then return g:win("Les oni ont eu raison de vous a la manche " .. wave .. ".") end
    if remaining > 0 then return end
    if wave >= WAVES then return g:win("Dix manches tenues : vous avez repousse les oni !") end
    pause = pause - 1
    if pause <= 0 then
      send_wave(g)
      pause = 8
    end
  end,
  status = function(g) return "manche " .. wave .. " / " .. WAVES .. " - " .. math.max(0, remaining) .. " oni" end }

on("mob_death", function() if the_game.phase == "jeu" then remaining = remaining - 1 end end)
