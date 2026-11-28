-- DiscoSheep : une piste de danse faite de dalles de quatre matieres. Une matiere est annoncee ; quelques
-- secondes plus tard, toutes les autres dalles disparaissent. Qui tombe est elimine. La piste se refait,
-- et tout va de plus en plus vite. Le dernier sur la piste gagne.
local TILES = { { "fourrure", "Fourrure" }, { "bloc_de_braise", "Braise" }, { "mousse", "Mousse" }, { "glace", "Glace" } }
local SIZE, CELL = 4, 4       -- 4 x 4 dalles de 4 blocs de cote
local layout, target, left, round, gone = {}, 1, 0, 0, false

local function paint(g, only)
  for i = 0, SIZE - 1 do
    for j = 0, SIZE - 1 do
      local t = layout[i * SIZE + j + 1]
      local x, z = g.ox - 8 + i * CELL, g.oz - 8 + j * CELL
      world.fill(x, g.oy, z, x + CELL - 1, g.oy, z + CELL - 1, (only and t ~= only) and "air" or TILES[t][1])
    end
  end
end

local function shuffle(g)
  for k = 1, SIZE * SIZE do layout[k] = g:roll(#TILES) end
  -- Chaque matiere est sur la piste au moins une fois.
  for t = 1, #TILES do layout[g:roll(SIZE * SIZE)] = t end
  for t = 1, #TILES do layout[(t * 5) % (SIZE * SIZE) + 1] = t end
end

local function next_round(g)
  round, gone = round + 1, false
  shuffle(g)
  paint(g)
  target = layout[g:roll(SIZE * SIZE)]
  left = math.max(2, 7 - math.floor(round / 2))
  announce("Tous sur : " .. TILES[target][2] .. " !")
end

minigame{ id = "discosheep", name = "DiscoSheep", min = 2, void = 6, pvp = false,
  build = function(g)
    round = 0
    shuffle(g)
    paint(g)
  end,
  spawn = function(g, p, i)
    local x, z = g:ring(i, 5)
    p:teleport(x + 0.5, g.oy + 1.2, z + 0.5)
  end,
  start = function(g) next_round(g) end,
  second = function(g)
    left = left - 1
    if left > 0 then return end
    if gone then
      next_round(g)
    else
      -- Les mauvaises dalles disparaissent ; la piste revient deux secondes plus tard.
      paint(g, target)
      gone, left = true, 2
    end
  end,
  status = function(g) return g.count .. " sur la piste - manche " .. round .. " - " .. TILES[target][2] end }
