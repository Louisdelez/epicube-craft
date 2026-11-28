-- UHC (Ultra Hardcore) : chacun pour soi dans un monde de survie. La vie ne remonte jamais toute seule :
-- seules la pomme doree et la tete d'un adversaire la rendent. Apres un temps de paix, le combat s'ouvre
-- et la zone retrecit jusqu'a forcer la rencontre. Le dernier en vie gagne.
-- Un mode de jeu est un mod comme un autre : ce fichier est tout ce qu'il y a a lire pour en faire un.

local MIN_PLAYERS = 2     -- joueurs qu'il faut pour lancer
local COUNTDOWN = 20      -- secondes entre « assez de joueurs » et le depart
local GRACE = 90          -- secondes sans combat entre joueurs, au debut
local ZONE_START = 260    -- demi-cote de la zone au depart, en blocs
local ZONE_END = 24       -- demi-cote de la zone a la fin
local SHRINK = 15 * 60    -- secondes que met la zone a retrecir

-- La pomme doree se fabrique (une pomme entouree d'or) ; la tete tombe d'un joueur vaincu.
items.register{ id = "pomme_doree", name = "Pomme doree", stack = 16 }
items.register{ id = "tete", name = "Tete de joueur", stack = 16 }
recipes.shaped{ output = "pomme_doree", count = 1, pattern = { "GGG", "GPG", "GGG" }, key = { G = "lingot_d_or", P = "pomme" } }
recipes.shaped{ output = "pomme_doree", count = 2, pattern = { "GGG", "GTG", "GGG" }, key = { G = "lingot_d_or", T = "tete" } }

local phase = "attente"   -- attente, jeu, fin
local clock, ticks = 0, 0
local wait = COUNTDOWN
local cx, cz = 0, 0       -- centre de la zone
local alive, count = {}, 0
local kills = {}
local started_with = 0

rules.set("regen", false)
rules.set("pvp", false)

local function zone()
  local k = math.min(1, math.max(0, (clock - GRACE) / SHRINK))
  return ZONE_START + (ZONE_END - ZONE_START) * k
end

local function clock_text(s)
  return string.format("%d:%02d", math.floor(s / 60), s % 60)
end

local function status(p)
  if phase == "attente" then
    local n = #players()
    if n < MIN_PLAYERS then return "UHC - en attente de joueurs (" .. n .. "/" .. MIN_PLAYERS .. ")" end
    return "UHC - depart dans " .. wait .. " s"
  elseif phase == "jeu" then
    local text = "UHC " .. clock_text(clock) .. " - " .. count .. " en vie - zone " .. math.floor(zone()) .. " blocs"
    if clock < GRACE then text = text .. " - combat dans " .. (GRACE - clock) .. " s" end
    local x, _, z = p:pos()
    if x and alive[p.name] then
      -- A quelle distance du bord de la zone se trouve le joueur.
      local edge = math.floor(zone() - math.max(math.abs(x - cx), math.abs(z - cz)))
      text = text .. " - bord a " .. edge .. " - " .. (kills[p.name] or 0) .. " victoire" .. ((kills[p.name] or 0) > 1 and "s" or "")
    end
    return text
  end
  return "UHC - partie terminee"
end

local function start()
  local list = players()
  local first = list[1]
  local x, _, z = first:pos()
  cx, cz = math.floor(x), math.floor(z)
  alive, count, kills = {}, 0, {}
  for i, p in ipairs(list) do
    -- Les joueurs sont repartis en cercle, loin les uns des autres.
    local a = (i - 1) / #list * 2 * math.pi
    local px, pz = cx + math.floor(math.cos(a) * ZONE_START * 0.7), cz + math.floor(math.sin(a) * ZONE_START * 0.7)
    local py = world.top(px, pz) or 150
    p:clear()
    p:set_mode("survie")
    p:set_health(20)
    p:set_hunger(20)
    p:teleport(px + 0.5, py + 1.5, pz + 0.5)
    p:give("onigiri", 3)
    alive[p.name] = true
    count = count + 1
  end
  started_with = count
  phase, clock = "jeu", 0
  world.set_time(7)
  announce("UHC : c'est parti ! Combat dans " .. GRACE .. " secondes.")
  say("La vie ne remonte pas toute seule : seules la pomme doree et la tete d'un adversaire la rendent.")
  say("La zone fait " .. ZONE_START .. " blocs autour de " .. cx .. ", " .. cz .. " et va retrecir jusqu'a " .. ZONE_END .. ".")
end

local function finish()
  phase = "fin"
  rules.set("pvp", false)
  local winner
  for name in pairs(alive) do winner = name end
  if winner then
    local n = kills[winner] or 0
    announce(winner .. " remporte l'UHC !")
    say(winner .. " remporte l'UHC avec " .. n .. " victoire" .. (n > 1 and "s" or "") .. " !")
  else
    announce("UHC termine : personne n'a survecu.")
  end
end

local function out(name, why)
  if phase == "jeu" and alive[name] then
    alive[name] = nil
    count = count - 1
    say(name .. " est elimine" .. why .. ". Il reste " .. count .. " joueur" .. (count > 1 and "s" or "") .. ".")
    if count <= 1 and started_with > 1 then finish() end
  end
end

on("join", function(p)
  if phase == "attente" then
    p:set_mode("aventure")
    p:tell("Bienvenue dans l'UHC ! La partie commence des que " .. MIN_PLAYERS .. " joueurs sont la.")
  else
    -- Arrive en cours de partie : on regarde.
    p:set_mode("spectateur")
    p:tell("La partie est en cours : vous la regardez en spectateur.")
  end
end)

on("leave", function(p) out(p.name, " (parti)") end)

on("death", function(p, killer)
  local by = killer ~= "" and killer or nil
  if phase == "jeu" and alive[p.name] then
    -- Le vaincu laisse sa tete : son vainqueur y gagnera de la vie.
    local x, y, z = p:pos()
    if x then world.drop("uhc:tete", 1, x, y + 1, z) end
    if by then kills[by] = (kills[by] or 0) + 1 end
    world.sound("explosion", x or cx, y or 100, z or cz, 0.6)
  end
  out(p.name, by and (" par " .. by) or "")
end)

-- Pas de seconde vie : on revient en spectateur.
on("respawn", function(p)
  if phase ~= "attente" then p:set_mode("spectateur") end
end)

-- La pomme doree rend quatre coeurs, la tete deux : c'est la seule facon de se soigner.
on("use", function(p, item)
  local heal = item == "uhc:pomme_doree" and 8 or item == "uhc:tete" and 4 or nil
  if not heal then return end
  if p:health() >= 20 then
    p:info("Vous etes deja en pleine forme.")
    return false
  end
  if p:take(item, 1) then
    p:set_health(math.min(20, p:health() + heal))
    p:sound("mange", 0.9)
  end
  return false
end)

on("tick", function()
  ticks = ticks + 1
  if ticks % 20 ~= 0 then return end
  local list = players()
  if phase == "attente" then
    if #list >= MIN_PLAYERS then
      wait = wait - 1
      if wait <= 0 then start() elseif wait <= 5 or wait % 5 == 0 then announce("Depart dans " .. wait .. " s") end
    else
      wait = COUNTDOWN
    end
  elseif phase == "jeu" then
    clock = clock + 1
    if clock == GRACE - 10 then announce("Le combat s'ouvre dans 10 secondes.") end
    if clock == GRACE then
      rules.set("pvp", true)
      announce("Le combat entre joueurs est ouvert ! La zone commence a retrecir.")
    end
    if clock == GRACE + SHRINK then
      announce("Mort subite : la zone ne fait plus que " .. ZONE_END .. " blocs.")
    end
    local r = zone()
    -- Hors de la zone, on perd de la vie : un demi-coeur par seconde, le double a la mort subite.
    local sting = clock >= GRACE + SHRINK and 2 or 1
    for _, p in ipairs(list) do
      if alive[p.name] then
        local x, _, z = p:pos()
        if x and (math.abs(x - cx) > r or math.abs(z - cz) > r) then
          p:damage(sting)
          p:info("Hors de la zone ! Revenez vers " .. cx .. ", " .. cz .. ".")
        end
      end
    end
    if clock % 120 == 0 then say("Zone : " .. math.floor(r) .. " blocs autour de " .. cx .. ", " .. cz .. ". " .. count .. " en vie.") end
  end
  for _, p in ipairs(list) do p:send("etat", status(p)) end
end)

command("uhc", "Lance l'UHC sans attendre (avec « demarrer »), ou dit ou en est la partie", function(p, args)
  if args == "demarrer" and phase == "attente" then
    start()
    return "UHC lance."
  end
  return status(p)
end)
