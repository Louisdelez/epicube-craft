-- BuildWar : un theme est tire au sort, chacun recoit une parcelle et tous les blocs du jeu. Trois minutes
-- pour batir ; puis l'on visite chaque parcelle, et chacun note celle des autres de 1 a 5 avec /note. La
-- meilleure moyenne gagne.
local BUILD, VISIT = 180, 15
local THEMES = { "un torii", "un chateau", "un dragon", "une maison de the", "un bateau", "un pont", "une lanterne geante", "un jardin zen", "une tour", "un monstre" }
local theme, owner, votes, stage, visiting, left = "", {}, {}, "batir", 0, BUILD

local function plot(g, i) return g.ox + (i - 1) * 22, g.oz end

local function visit(g, k)
  visiting, left = k, VISIT
  local x, z = plot(g, k)
  for _, p in ipairs(players()) do p:teleport(x + 0.5, g.oy + 3, z - 9.5) end
  announce("Parcelle de " .. owner[k] .. " : notez-la avec /note 1 a 5.")
end

local the_game
the_game = minigame{ id = "buildwar", name = "BuildWar", min = 2, pvp = false, endless = true, mode = "creatif", places = 12,
  build = function(g)
    theme, owner, votes, stage, visiting, left = THEMES[g:roll(#THEMES)], {}, {}, "batir", 0, BUILD
  end,
  spawn = function(g, p, i)
    local x, z = plot(g, i)
    owner[i], votes[i] = p.name, {}
    world.fill(x - 8, g.oy, z - 8, x + 8, g.oy, z + 8, "obsidienne")
    world.fill(x - 7, g.oy, z - 7, x + 7, g.oy, z + 7, "herbe")
    p:teleport(x + 0.5, g.oy + 1.2, z + 0.5)
  end,
  start = function(g)
    announce("Theme : " .. theme .. " ! Vous avez " .. BUILD .. " secondes.")
    say("Theme du BuildWar : " .. theme .. ".")
  end,
  death = function() return true end,
  second = function(g)
    left = left - 1
    if stage == "batir" then
      if left == 30 then announce("Plus que 30 secondes !") end
      if left <= 0 then
        stage = "noter"
        for _, p in ipairs(g:players()) do p:set_mode("aventure") end
        visit(g, 1)
      end
    elseif left <= 0 then
      if visiting < #owner then
        visit(g, visiting + 1)
      else
        -- Le decompte : la meilleure moyenne l'emporte.
        local best, who = -1, nil
        for k, name in ipairs(owner) do
          local sum, n = 0, 0
          for _, v in pairs(votes[k]) do sum, n = sum + v, n + 1 end
          local avg = n > 0 and sum / n or 0
          say(name .. " : " .. string.format("%.1f", avg) .. " / 5")
          if avg > best then best, who = avg, name end
        end
        g:win((who or "Personne") .. " remporte le BuildWar sur le theme « " .. theme .. " » !")
      end
    end
  end,
  status = function(g)
    if stage == "batir" then return "theme : " .. theme .. " - " .. math.max(0, left) .. " s" end
    return "vote : parcelle de " .. (owner[visiting] or "?") .. " - " .. math.max(0, left) .. " s"
  end }

command("note", "Note de 1 a 5 la parcelle que l'on visite", function(p, args)
  local n = tonumber(args)
  if the_game.phase ~= "jeu" or stage ~= "noter" then return "Le vote n'est pas ouvert." end
  if not n or n < 1 or n > 5 then return "Une note de 1 a 5." end
  if owner[visiting] == p.name then return "On ne note pas sa propre parcelle." end
  votes[visiting][p.name] = math.floor(n)
  return "Vous donnez " .. math.floor(n) .. " a la parcelle de " .. owner[visiting] .. "."
end)

-- Chacun ne batit que sur sa parcelle, et seulement pendant le temps de construction.
local function mine(p, x, z)
  local g = the_game
  if g.phase ~= "jeu" or stage ~= "batir" then return false end
  for k, name in ipairs(owner) do
    local px, pz = plot(g, k)
    if name == p.name then return math.abs(x - px) <= 7 and math.abs(z - pz) <= 7 end
  end
  return false
end
on("place", function(p, x, y, z) if not mine(p, x, z) then return false end end)
on("break", function(p, x, y, z) if not mine(p, x, z) then return false end end)
