-- PvPSwap (second jeu d'Epicube, de Trifcan, inspire du Death Swap de SethBling : « le terrain se compose de
-- differents modules ou les joueurs sont regulierement interchanges ») : chacun commence sur son ile, avec du
-- bois, de la pierre, du minerai et un coffre. On s'equipe comme on peut ; toutes les quarante secondes tout
-- le monde change d'ile. Chaque adversaire vaincu laisse sa tete : s'en servir rend trois coeurs. Tomber dans
-- le vide tue. Le dernier debout gagne.
local SWAP = 40
items.register{ id = "tete", name = "Tete de joueur", stack = 16 }
local spots = {}

local function module(g, i)
  local x, z = g:ring(i, 26)
  world.island(x, g.oy, z, 5, "herbe")
  world.fill(x - 3, g.oy + 1, z - 3, x - 3, g.oy + 4, z - 3, "tronc_de_cedre")
  world.fill(x - 4, g.oy + 5, z - 4, x - 2, g.oy + 6, z - 2, "feuilles")
  world.fill(x + 2, g.oy + 1, z + 2, x + 4, g.oy + 2, z + 4, "pierre")
  world.set(x + 3, g.oy + 1, z + 3, "minerai_de_fer")
  world.set(x + 4, g.oy + 2, z + 3, "minerai_de_fer")
  world.set(x + 2, g.oy + 1, z + 4, "minerai_de_charbon")
  world.set(x + 3, g.oy + 3, z + 3, "etabli")
  world.set(x + 2, g.oy + 3, z + 3, "fourneau_kamado")
  local loot = { { "pomme", 2 }, { "baton", 4 }, { "fleche", 6 }, { "cuir", 3 }, { "lingot_de_fer", 2 }, { "planches_de_hinoki", 12 } }
  for k = 1, 3 do
    local item = loot[g:roll(#loot)]
    world.chest(x - 2, g.oy + 1, z + 3, item[1], item[2])
  end
  return x, z
end

local the_game
the_game = minigame{ id = "pvpswap", name = "PvPSwap", min = 2, void = 20, mode = "survie",
  build = function(g) spots = {} end,
  spawn = function(g, p, i)
    local x, z = module(g, i)
    spots[#spots + 1] = { x, z }
    p:teleport(x + 0.5, g.oy + 1.2, z + 0.5)
    p:give("bokken", 1)
    p:give("pioche_en_bois", 1)
    p:give("onigiri", 4)
  end,
  death = function(g, p)
    local x, y, z = p:pos()
    if x then world.drop("pvpswap:tete", 1, x, y + 1, z) end
  end,
  second = function(g, clock)
    local left = SWAP - clock % SWAP
    if left == 5 then announce("Echange dans 5 secondes !") end
    if clock % SWAP ~= 0 then return end
    -- Tout le monde prend la place du suivant.
    local list, at = g:players(), {}
    for i, p in ipairs(list) do at[i] = { p:pos() } end
    for i, p in ipairs(list) do
      local to = at[i % #list + 1]
      if to[1] then p:teleport(to[1], to[2] + 0.2, to[3]) end
    end
    announce("Echange !")
  end,
  status = function(g) return g.count .. " en vie - echange dans " .. (SWAP - g.clock % SWAP) .. " s" end }

on("use", function(p, item)
  if item ~= "pvpswap:tete" then return end
  if p:take(item, 1) then
    p:set_health(math.min(20, (p:health() or 20) + 6))
    p:sound("mange", 0.9)
  end
  return false
end)
