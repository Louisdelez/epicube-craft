-- Totem (d'apres Epicube : « deux equipes s'affrontent dans le but de detruire le totem adverse ») : chaque
-- base a son totem, une colonne de cinq blocs. On le defend, on batit, on va abattre celui d'en face a la
-- hache. Le dernier bloc tombe : la partie est gagnee. On revient en jeu apres chaque mort.
local HEIGHT_OF = 5
blocks.register{ id = "totem", name = "Totem", hardness = 3, tool = "hache", drop = false }
local left, placed = { HEIGHT_OF, HEIGHT_OF }, {}

local function base(g, team) return g.ox + (team == 1 and -22 or 22), g.oz end
local function key(x, y, z) return x .. "," .. y .. "," .. z end

local function kit(p)
  p:give("katana_en_pierre", 1)
  p:give("hache_ono_en_pierre", 1)
  p:give("fourrure", 32)
  p:give("arc_yumi", 1)
  p:give("fleche", 6)
  p:give("onigiri", 3)
end

local the_game
the_game = minigame{ id = "totem", name = "Totem", min = 2, teams = { "Rouge", "Bleue" }, void = 16, endless = true, mode = "survie",
  build = function(g)
    left, placed = { HEIGHT_OF, HEIGHT_OF }, {}
    world.fill(g.ox - 22, g.oy, g.oz - 2, g.ox + 22, g.oy, g.oz + 2, "pierre_de_muraille")
    world.island(g.ox, g.oy, g.oz, 5, "herbe")
    for team = 1, 2 do
      local bx, bz = base(g, team)
      world.island(bx, g.oy, bz, 6, "herbe")
      world.fill(bx, g.oy + 1, bz, bx, g.oy + HEIGHT_OF, bz, "totem:totem")
    end
  end,
  spawn = function(g, p, i, team)
    local bx, bz = base(g, team)
    p:set_home(bx + 0.5, g.oy + 2, bz + 3.5)
    p:teleport(bx + 0.5, g.oy + 1.2, bz + 3.5)
    kit(p)
  end,
  death = function() return true end,
  respawn = function(g, p) kit(p) end,
  status = function(g) return "totem rouge " .. left[1] .. " / " .. HEIGHT_OF .. " - totem bleu " .. left[2] .. " / " .. HEIGHT_OF end }

-- Seuls se cassent les blocs poses par les joueurs, et le totem adverse.
on("break", function(p, x, y, z, block)
  local g = the_game
  if g.phase ~= "jeu" then return false end
  if block == "totem:totem" then
    local whose = x < g.ox and 1 or 2
    if g:team(p) == whose then
      p:info("C'est votre totem !")
      return false
    end
    left[whose] = left[whose] - 1
    announce("Le totem de l'equipe " .. g:team_name(whose) .. " perd un bloc (" .. left[whose] .. " restant" .. (left[whose] > 1 and "s" or "") .. ").")
    if left[whose] <= 0 then g:win("L'equipe " .. g:team_name(whose == 1 and 2 or 1) .. " abat le totem adverse et remporte la partie !") end
    return true
  end
  local k = key(x, y, z)
  if placed[k] then
    placed[k] = nil
    return true
  end
  return false
end)
on("place", function(p, x, y, z)
  local g = the_game
  if g.phase ~= "jeu" or math.abs(x - g.ox) > 32 or math.abs(z - g.oz) > 14 or y > g.oy + 12 or y < g.oy - 8 then return false end
  placed[key(x, y, z)] = true
end)
