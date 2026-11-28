-- Catch The Cow (d'apres le jeu de Likaos sur Epicube, inspire du Goulet des Chanteguerres) : chaque equipe
-- garde une vache dans sa base. On va prendre celle d'en face en marchant dessus, on la ramene chez soi :
-- un point. Qui tombe en la portant la laisse repartir. La premiere equipe a trois points gagne.
local GOAL = 3
blocks.register{ id = "vache", name = "Vache", hardness = -1, light = 8 }
local FLAG = "catchthecow:vache"
local score, carried = { 0, 0 }, {}     -- carried[equipe] : qui porte le trophee de cette equipe

local function kit(p)
  p:give("katana_en_fer", 1)
  p:give("arc_yumi", 1)
  p:give("fleche", 8)
  p:give("cuirasse_do_de_bambou", 1)
  p:give("onigiri", 3)
end

local function base(g, team) return g.ox + (team == 1 and -24 or 24), g.oz end

local function plant(g, team)
  local bx, bz = base(g, team)
  world.set(bx, g.oy + 1, bz, FLAG)
  carried[team] = nil
end

minigame{ id = "catchthecow", name = "Catch The Cow", min = 2, teams = { "Rouge", "Bleue" }, void = 14, endless = true,
  build = function(g)
    score, carried = { 0, 0 }, {}
    -- Deux bases, un terre-plein au milieu, et une chaussee qui les relie.
    world.fill(g.ox - 24, g.oy, g.oz - 2, g.ox + 24, g.oy, g.oz + 2, "pierre_de_muraille")
    world.island(g.ox, g.oy, g.oz, 6, "herbe")
    for team = 1, 2 do
      local bx, bz = base(g, team)
      world.island(bx, g.oy, bz, 5, team == 1 and "bloc_de_braise" or "bloc_de_reiseki")
      plant(g, team)
    end
    for _, o in ipairs({ { -3, -3 }, { 3, 3 }, { -3, 3 }, { 3, -3 } }) do
      world.fill(g.ox + o[1], g.oy + 1, g.oz + o[2], g.ox + o[1], g.oy + 2, g.oz + o[2], "tronc_de_cedre")
    end
  end,
  spawn = function(g, p, i, team)
    local bx, bz = base(g, team)
    p:set_home(bx + 0.5, g.oy + 2, bz + 2.5)
    p:teleport(bx + 0.5, g.oy + 1.2, bz + 2.5 + (i % 2))
    kit(p, i)
  end,
  death = function(g, p)
    -- Qui tombe avec le trophee le laisse repartir chez lui.
    for team = 1, 2 do
      if carried[team] == p.name then
        plant(g, team)
        announce("Le trophee de l'equipe " .. g:team_name(team) .. " retourne a sa base.")
      end
    end
    return true
  end,
  respawn = function(g, p) kit(p, 0) end,
  second = function(g)
    for _, p in ipairs(g:players()) do
      local mine = g:team(p)
      local theirs = mine == 1 and 2 or 1
      local x, _, z = p:pos()
      if x then
        local ex, ez = base(g, theirs)
        local hx, hz = base(g, mine)
        if not carried[theirs] and math.abs(x - ex - 0.5) < 1.6 and math.abs(z - ez - 0.5) < 1.6 then
          carried[theirs] = p.name
          world.set(ex, g.oy + 1, ez, "air")
          announce(p.name .. " emporte le trophee de l'equipe " .. g:team_name(theirs) .. " !")
        elseif carried[theirs] == p.name and math.abs(x - hx - 0.5) < 2.5 and math.abs(z - hz - 0.5) < 2.5 then
          score[mine] = score[mine] + 1
          plant(g, theirs)
          announce("Point pour l'equipe " .. g:team_name(mine) .. " ! (" .. score[1] .. " - " .. score[2] .. ")")
          if score[mine] >= GOAL then g:win("L'equipe " .. g:team_name(mine) .. " remporte la partie de Catch The Cow !") end
        end
      end
    end
  end,
  status = function(g, p)
    local text = "Rouge " .. score[1] .. " - Bleue " .. score[2] .. " (en " .. GOAL .. ")"
    for team = 1, 2 do if carried[team] == p.name then text = text .. " - ramenez le trophee a votre base !" end end
    return text
  end }
