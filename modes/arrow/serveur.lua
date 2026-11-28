-- Arrow : un arc, une seule fleche, et elle tue d'un coup. Chaque victoire rend une fleche ; une fleche
-- manquee est perdue, il reste le sabre de bois. On revient apres chaque mort. Le premier a 10 gagne.
local GOAL = 10

local function kit(p)
  p:give_enchanted("arc_yumi", "puissance", 5)
  p:give("bokken", 1)
  p:give("fleche", 1)
end

local function named(name)
  for _, p in ipairs(players()) do if p.name == name then return p end end
end

minigame{ id = "arrow", name = "Arrow", min = 2, endless = true,
  build = function(g)
    world.fill(g.ox - 12, g.oy, g.oz - 12, g.ox + 12, g.oy, g.oz + 12, "herbe")
    for _, w in ipairs({ { -12, -12, 12, -12 }, { -12, 12, 12, 12 }, { -12, -12, -12, 12 }, { 12, -12, 12, 12 } }) do
      world.fill(g.ox + w[1], g.oy + 1, g.oz + w[2], g.ox + w[3], g.oy + 3, g.oz + w[4], "tronc_de_cedre")
    end
    -- De quoi se cacher : quelques murets.
    for _, o in ipairs({ { -6, 0 }, { 6, 0 }, { 0, -6 }, { 0, 6 }, { -3, -3 }, { 3, 3 } }) do
      world.fill(g.ox + o[1], g.oy + 1, g.oz + o[2], g.ox + o[1] + 1, g.oy + 2, g.oz + o[2], "pierre_de_muraille")
    end
  end,
  spawn = function(g, p, i)
    local x, z = g:ring(i, 9)
    p:set_home(x + 0.5, g.oy + 2, z + 0.5)
    p:teleport(x + 0.5, g.oy + 1.2, z + 0.5)
    kit(p)
  end,
  death = function(g, p, by)
    local killer = by and named(by)
    if killer then
      killer:give("fleche", 1)
      if (g.kills[by] or 0) >= GOAL then g:win(by .. " remporte la partie d'Arrow !") end
    end
    return true
  end,
  respawn = function(g, p) kit(p) end,
  status = function(g, p)
    local best, who = 0, "personne"
    for name, k in pairs(g.kills) do if k > best then best, who = k, name end end
    return "vous " .. (g.kills[p.name] or 0) .. " / " .. GOAL .. " - en tete : " .. who .. " " .. best
  end }
