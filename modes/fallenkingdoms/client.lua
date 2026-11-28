-- Fallen Kingdoms, cote ecran : l'etat de la partie, en haut au milieu.
local text = ""
on("message", function(name, data) if name == "etat" then text = data end end)
on("hud", function()
  if text == "" then return end
  local w = draw.screen()
  local tw = draw.text_width(text)
  draw.rect((w - tw) / 2 - 5, 4, tw + 10, 13, 0, 0, 0, 0.55)
  draw.text((w - tw) / 2, 7, text, 1, 0.86, 0.36, 1)
end)
