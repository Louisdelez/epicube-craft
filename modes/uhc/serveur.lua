-- UHC : chacun pour soi. La vie ne remonte pas toute seule, la zone retrecit, le dernier en vie gagne.
-- Un mode de jeu est un mod comme un autre : ce fichier est tout ce qu'il y a a lire pour en faire un.

local MIN_PLAYERS = 2     -- joueurs qu'il faut pour lancer
local COUNTDOWN = 20      -- secondes entre « assez de joueurs » et le depart
local GRACE = 90          -- secondes sans combat entre joueurs, au debut
local ZONE_START = 260    -- demi-cote de la zone au depart, en blocs
local ZONE_END = 24       -- demi-cote de la zone a la fin
local SHRINK = 15 * 60    -- secondes que met la zone a retrecir

local phase = "attente"   -- attente, jeu, fin
local clock, ticks = 0, 0
local wait = COUNTDOWN
local cx, cz = 0, 0       -- centre de la zone
local alive, count = {}, 0
local started_with = 0

rules.set("regen", false)
rules.set("pvp", false)

local function zone()
  local k = math.min(1, math.max(0, (clock - GRACE) / SHRINK))
  return ZONE_START + (ZONE_END - ZONE_START) * k
end

local function status(p)
  if phase == "attente" then
    local n = #players()
    if n < MIN_PLAYERS then return "UHC - en attente de joueurs (" .. n .. "/" .. MIN_PLAYERS .. ")" end
    return "UHC - depart dans " .. wait .. " s"
  elseif phase == "jeu" then
    local text = "UHC - " .. count .. " en vie - zone " .. math.floor(zone()) .. " blocs"
    if clock < GRACE then text = text .. " - combat dans " .. (GRACE - clock) .. " s" end
    return text
  end
  return "UHC - partie terminee"
end

local function start()
  local list = players()
  local first = list[1]
  local x, _, z = first:pos()
  cx, cz = math.floor(x), math.floor(z)
  alive, count = {}, 0
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
  announce("UHC : c'est parti ! Combat dans " .. GRACE .. " secondes.")
  say("La vie ne remonte pas toute seule. La zone fait " .. ZONE_START .. " blocs autour du centre et va retrecir.")
end

local function finish()
  phase = "fin"
  rules.set("pvp", false)
  local winner
  for name in pairs(alive) do winner = name end
  if winner then
    announce(winner .. " remporte l'UHC !")
    say(winner .. " remporte l'UHC !")
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
  out(p.name, (killer ~= "" and killer) and (" par " .. killer) or "")
end)

-- Pas de seconde vie : on revient en spectateur.
on("respawn", function(p)
  if phase ~= "attente" then p:set_mode("spectateur") end
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
    if clock == GRACE then
      rules.set("pvp", true)
      announce("Le combat entre joueurs est ouvert !")
    end
    local r = zone()
    for _, p in ipairs(list) do
      if alive[p.name] then
        local x, _, z = p:pos()
        if x and (math.abs(x - cx) > r or math.abs(z - cz) > r) then
          p:damage(1)
          p:info("Hors de la zone ! Revenez vers le centre.")
        end
      end
    end
    if clock % 120 == 0 then say("Zone : " .. math.floor(r) .. " blocs autour de " .. cx .. ", " .. cz .. ".") end
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
