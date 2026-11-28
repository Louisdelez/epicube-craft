-- SheepWars (le premier jeu d'Epicube, de Likaos : « deux equipes s'affrontent avec leurs arcs, epees... et
-- moutons aux effets tous plus delirants les uns que les autres ») : deux navires face a face dans le ciel.
-- Chacun a un arc, un sabre, une armure, et recoit regulierement un mouton : un clic droit le lance.
-- Rouge : explosif. Rose : soigneur. Jaune : foudre. Blanc : abordage, il vous porte sur le pont ennemi.
-- Pas de seconde vie : la derniere equipe a flot gagne. (Ici le mouton va de lui-meme a l'ennemi le plus proche.)
local SHEEP = {
  { "mouton_explosif", "Mouton explosif" }, { "mouton_soigneur", "Mouton soigneur" },
  { "mouton_foudre", "Mouton foudre" }, { "mouton_abordage", "Mouton d'abordage" },
}
for _, s in ipairs(SHEEP) do items.register{ id = s[1], name = s[2], stack = 8 } end
local fuses = {}

local function deck(g, team) return g.ox + (team == 1 and -14 or 14), g.oz end

-- L'ennemi le plus proche du joueur : lui, et sa distance.
local function nearest_enemy(g, p)
  local x, y, z = p:pos()
  local best, bd
  for _, o in ipairs(g:players()) do
    if g:team(o) ~= g:team(p) then
      local ox, oy, oz = o:pos()
      if x and ox then
        local d = (ox - x) ^ 2 + (oy - y) ^ 2 + (oz - z) ^ 2
        if not bd or d < bd then best, bd = o, d end
      end
    end
  end
  return best
end

local the_game
the_game = minigame{ id = "sheepwars", name = "SheepWars", min = 2, teams = { "Rouge", "Bleue" }, void = 16,
  build = function(g)
    fuses = {}
    for team = 1, 2 do
      local dx, dz = deck(g, team)
      -- Le navire : un pont, un bastingage du cote du large, un mat.
      world.fill(dx - 4, g.oy, dz - 9, dx + 4, g.oy, dz + 9, "planches_de_hinoki")
      world.fill(dx - 3, g.oy - 1, dz - 7, dx + 3, g.oy - 1, dz + 7, "planches_de_hinoki")
      local out = team == 1 and -4 or 4
      world.fill(dx + out, g.oy + 1, dz - 9, dx + out, g.oy + 1, dz + 9, "tronc_de_cedre")
      world.fill(dx, g.oy + 1, dz, dx, g.oy + 7, dz, "tronc_de_cedre")
      world.fill(dx, g.oy + 4, dz - 3, dx, g.oy + 7, dz + 3, "fourrure")
    end
  end,
  spawn = function(g, p, i, team)
    local dx, dz = deck(g, team)
    p:teleport(dx + 0.5, g.oy + 1.2, dz + ((i * 3) % 15) - 7 + 0.5)
    p:give("arc_yumi", 1)
    p:give("fleche", 24)
    p:give("katana_en_pierre", 1)
    p:give("cuirasse_do_de_bambou", 1)
    p:give("sheepwars:mouton_explosif", 1)
  end,
  second = function(g, clock)
    -- Les moutons explosifs lances arrivent a destination.
    for k = #fuses, 1, -1 do
      local f = fuses[k]
      f.left = f.left - 1
      if f.left <= 0 then
        world.explode(f.x, f.y, f.z, 2.5)
        table.remove(fuses, k)
      end
    end
    if clock % 12 == 0 then
      for _, p in ipairs(g:players()) do p:give("sheepwars:" .. SHEEP[g:roll(#SHEEP)][1], 1) end
    end
  end,
  status = function(g) return "Rouge " .. g:mates(1) .. " - Bleue " .. g:mates(2) .. " - mouton dans " .. (12 - g.clock % 12) .. " s" end }

on("use", function(p, item)
  local g = the_game
  if item:sub(1, 17) ~= "sheepwars:mouton_" then return end
  if g.phase ~= "jeu" or not g:alive(p) or not p:take(item, 1) then return false end
  local kind = item:sub(18)
  local foe = nearest_enemy(g, p)
  p:sound("clic", 0.8)
  if kind == "soigneur" then
    for _, o in ipairs(g:players()) do
      if g:team(o) == g:team(p) then o:set_health(math.min(20, (o:health() or 20) + 6)) end
    end
    p:info("Votre equipe est soignee.")
  elseif not foe then
    p:info("Aucun ennemi a portee.")
  elseif kind == "explosif" then
    local x, y, z = foe:pos()
    fuses[#fuses + 1] = { x = x, y = y, z = z, left = 2 }
    foe:info("Un mouton explosif arrive !")
  elseif kind == "foudre" then
    foe:damage(5)
    local x, y, z = foe:pos()
    world.sound("explosion", x, y, z, 0.8)
    foe:info("La foudre vous frappe !")
  elseif kind == "abordage" then
    local dx, dz = deck(g, g:team(foe))
    p:teleport(dx + 0.5, g.oy + 1.2, dz + 0.5)
    p:info("A l'abordage !")
  end
  return false
end)
