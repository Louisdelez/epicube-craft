-- Bienvenue : un petit mod en Lua. Rien a compiler, ce fichier suffit.

on("join", function(player)
  say(player.name .. " vient d'arriver, bienvenue !")
end)

command("soin", "Rend toute sa vie", function(player)
  player:set_health(20)
  return "Te voila gueri."
end)

command("horloge", "Dit l'heure du monde", function()
  return string.format("Il est %02dh", math.floor(world.time()))
end)
