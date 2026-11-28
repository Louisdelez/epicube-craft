-- PayDay (d'apres le jeu de Xstoudi puis Likaos sur Epicube, inspire du jeu du meme nom : « vous, voleur,
-- devez ramener un butin en cherchant de l'argent dans differents coffres-forts ») : les braqueurs vont a la
-- banque, vident ses coffres et rapportent les lingots a leur fourgon. Les policiers les en empechent. Les
-- braqueurs n'ont qu'une vie, les policiers reviennent. Dix lingots au fourgon avant la fin : les braqueurs
-- gagnent ; sinon, ou s'ils sont tous arretes, les policiers l'emportent.
local GOAL, TIME = 10, 480
local loot = 0

local function van(g) return g.ox - 26, g.oz end

minigame{ id = "payday", name = "PayDay", min = 2, teams = { "Braqueurs", "Policiers" }, void = 14, limit = TIME,
  build = function(g)
    loot = 0
    world.fill(g.ox - 30, g.oy, g.oz - 10, g.ox + 30, g.oy, g.oz + 10, "pierre_de_muraille")
    -- La banque : quatre murs, deux portes, quatre coffres-forts.
    world.fill(g.ox - 5, g.oy + 1, g.oz - 5, g.ox + 5, g.oy + 4, g.oz + 5, "pierre")
    world.fill(g.ox - 4, g.oy + 1, g.oz - 4, g.ox + 4, g.oy + 3, g.oz + 4, "air")
    world.fill(g.ox - 5, g.oy + 1, g.oz, g.ox - 5, g.oy + 2, g.oz, "air")
    world.fill(g.ox + 5, g.oy + 1, g.oz, g.ox + 5, g.oy + 2, g.oz, "air")
    for _, o in ipairs({ { -3, -3 }, { 3, 3 }, { -3, 3 }, { 3, -3 } }) do
      for _ = 1, 2 do world.chest(g.ox + o[1], g.oy + 1, g.oz + o[2], "lingot_d_or", 2) end
    end
    local vx, vz = van(g)
    world.fill(vx - 2, g.oy, vz - 2, vx + 2, g.oy, vz + 2, "obsidienne")
  end,
  spawn = function(g, p, i, team)
    local x = team == 1 and (g.ox - 26) or (g.ox + 26)
    p:set_home(x + 0.5, g.oy + 2, g.oz + 0.5)
    p:teleport(x + 0.5, g.oy + 1.2, g.oz + ((i * 2) % 7) - 3 + 0.5)
    if team == 1 then
      p:give("katana_en_pierre", 1)
      p:give("bouclier_tate", 1)
      p:tell("Videz les coffres de la banque et rapportez les lingots sur la dalle d'obsidienne du fourgon.")
    else
      p:give("katana_en_fer", 1)
      p:give("cuirasse_do_de_fer", 1)
      p:tell("Empechez les braqueurs de rapporter dix lingots a leur fourgon.")
    end
    p:give("onigiri", 3)
  end,
  -- Un policier revient ; un braqueur arrete ne revient pas.
  death = function(g, p) return g:team(p) == 2 end,
  respawn = function(g, p)
    p:give("katana_en_fer", 1)
    p:give("cuirasse_do_de_fer", 1)
  end,
  second = function(g)
    local vx, vz = van(g)
    for _, p in ipairs(g:players()) do
      local x, _, z = p:pos()
      if g:team(p) == 1 and x and math.abs(x - vx - 0.5) <= 2.5 and math.abs(z - vz - 0.5) <= 2.5 then
        local n = p:count("lingot_d_or") or 0
        if n > 0 and p:take("lingot_d_or", n) then
          loot = loot + n
          announce(p.name .. " charge " .. n .. " lingot" .. (n > 1 and "s" or "") .. " dans le fourgon (" .. loot .. " / " .. GOAL .. ").")
          if loot >= GOAL then g:win("Les braqueurs repartent avec le butin : ils remportent PayDay !") end
        end
      end
    end
  end,
  timeout = function(g) g:win("Le temps est ecoule : les policiers ont protege la banque et remportent PayDay !") end,
  status = function(g) return "butin " .. loot .. " / " .. GOAL .. " - " .. g:mates(1) .. " braqueur(s) libre(s) - " .. math.max(0, TIME - g.clock) .. " s" end }
