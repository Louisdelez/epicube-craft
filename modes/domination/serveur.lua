-- Domination (premier jeu du Champ de Bataille d'Epicube, de Likaos) : trois points sur le terrain. On en
-- prend un en y etant plus nombreux que l'adversaire pendant cinq secondes ; chaque point tenu rapporte
-- un point par seconde. La premiere equipe a 200 gagne. On revient en jeu apres chaque mort.
local GOAL = 200
local score, owner, hold = { 0, 0 }, { 0, 0, 0 }, { 0, 0, 0 }
local MARK = { [0] = "pierre", "bloc_de_braise", "bloc_de_reiseki" }

local function point(g, i) return g.ox, g.oz + (i - 2) * 12 end

local function kit(p)
  p:give("katana_en_fer", 1)
  p:give("arc_yumi", 1)
  p:give("fleche", 8)
  p:give("cuirasse_do_de_bambou", 1)
  p:give("onigiri", 3)
end

minigame{ id = "domination", name = "Domination", min = 2, teams = { "Rouge", "Bleue" }, void = 14, endless = true,
  build = function(g)
    score, owner, hold = { 0, 0 }, { 0, 0, 0 }, { 0, 0, 0 }
    world.fill(g.ox - 28, g.oy, g.oz - 16, g.ox + 28, g.oy, g.oz + 16, "herbe")
    for i = 1, 3 do
      local px, pz = point(g, i)
      world.fill(px - 2, g.oy, pz - 2, px + 2, g.oy, pz + 2, "tuiles_kawara")
      world.set(px, g.oy, pz, "pierre")
    end
    for _, o in ipairs({ { -9, -6 }, { 9, 6 }, { -9, 6 }, { 9, -6 }, { -14, 0 }, { 14, 0 } }) do
      world.fill(g.ox + o[1], g.oy + 1, g.oz + o[2], g.ox + o[1], g.oy + 2, g.oz + o[2] + 1, "pierre_de_muraille")
    end
  end,
  spawn = function(g, p, i, team)
    local bx = g.ox + (team == 1 and -25 or 25)
    p:set_home(bx + 0.5, g.oy + 2, g.oz + 0.5)
    p:teleport(bx + 0.5, g.oy + 1.2, g.oz + ((i * 2) % 9) - 4 + 0.5)
    kit(p)
  end,
  death = function() return true end,
  respawn = function(g, p) kit(p) end,
  second = function(g)
    for i = 1, 3 do
      local px, pz = point(g, i)
      local on = { 0, 0 }
      for _, p in ipairs(g:players()) do
        local x, _, z = p:pos()
        if x and math.abs(x - px - 0.5) <= 2.5 and math.abs(z - pz - 0.5) <= 2.5 then on[g:team(p)] = on[g:team(p)] + 1 end
      end
      local lead = on[1] > on[2] and 1 or on[2] > on[1] and 2 or nil
      if lead and lead ~= owner[i] then
        hold[i] = hold[i] + 1
        if hold[i] >= 5 then
          owner[i], hold[i] = lead, 0
          world.set(px, g.oy, pz, MARK[lead])
          announce("L'equipe " .. g:team_name(lead) .. " prend le point " .. string.char(64 + i) .. " !")
        end
      else
        hold[i] = 0
      end
      if owner[i] > 0 then score[owner[i]] = score[owner[i]] + 1 end
    end
    for team = 1, 2 do
      if score[team] >= GOAL then g:win("L'equipe " .. g:team_name(team) .. " remporte la Domination " .. score[1] .. " a " .. score[2] .. " !") end
    end
  end,
  status = function(g)
    local held = ""
    for i = 1, 3 do held = held .. string.char(64 + i) .. ":" .. (owner[i] == 0 and "-" or owner[i] == 1 and "R" or "B") .. " " end
    return "Rouge " .. score[1] .. " - Bleue " .. score[2] .. " / " .. GOAL .. " - " .. held
  end }
