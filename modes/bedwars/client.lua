-- Bed Wars, cote ecran : l'etat des equipes en haut, et la fenetre de la boutique.
local SHOP = {
  { "fourrure", "16 blocs de fourrure", "4 fer" },
  { "planches_de_hinoki", "8 planches", "6 fer" },
  { "katana_en_pierre", "Katana en pierre", "10 fer" },
  { "pioche_en_bois", "Pioche en bois", "8 fer" },
  { "onigiri", "4 onigiri", "3 fer" },
  { "katana_en_fer", "Katana en fer", "3 diamants" },
  { "cuirasse_do_de_fer", "Cuirasse de fer", "4 diamants" },
  { "obsidienne", "4 blocs d'obsidienne", "4 diamants" },
}
local state, open, note = "", false, ""

local function row(i)
  local w, h = draw.screen()
  return (w - 240) / 2 + 8, (h - 220) / 2 + 26 + (i - 1) * 22, 224, 20
end

on("message", function(name, data)
  if name == "etat" then state = data
  elseif name == "boutique" then
    open, note = true, ""
    screen.open()
  elseif name == "achat" then
    note = data == "ok" and "Achete !" or "Pas assez de quoi payer."
  end
end)

on("close", function() open = false end)

on("click", function(x, y, button)
  if not open or button ~= 0 then return end
  for i = 1, #SHOP do
    local rx, ry, rw, rh = row(i)
    if x >= rx and x < rx + rw and y >= ry and y < ry + rh then send("acheter", i) end
  end
end)

on("hud", function()
  local w, h = draw.screen()
  if state ~= "" then
    local tw = draw.text_width(state)
    draw.rect((w - tw) / 2 - 5, 4, tw + 10, 13, 0, 0, 0, 0.55)
    draw.text((w - tw) / 2, 7, state, 1, 0.86, 0.36, 1)
  end
  if not open then return end
  local x, y = (w - 240) / 2, (h - 220) / 2
  draw.rect(x - 1, y - 1, 242, 222, 0.3, 0.2, 0.05, 1)
  draw.rect(x, y, 240, 220, 0.09, 0.08, 0.12, 0.96)
  draw.text(x + 8, y + 8, "Boutique", 1, 0.86, 0.36, 1)
  local mx, my = screen.mouse()
  for i, item in ipairs(SHOP) do
    local rx, ry, rw, rh = row(i)
    local hot = mx >= rx and mx < rx + rw and my >= ry and my < ry + rh
    if hot then draw.rect(rx, ry, rw, rh, 0.85, 0.65, 0.16, 0.9) else draw.rect(rx, ry, rw, rh, 0.2, 0.18, 0.26, 0.9) end
    draw.icon(rx + 3, ry + 2, 16, item[1])
    draw.text(rx + 24, ry + 6, item[2])
    draw.text(rx + rw - draw.text_width(item[3]) - 6, ry + 6, item[3], 0.8, 0.82, 0.9, 1)
  end
  draw.text(x + 8, y + 206, note, 0.6, 1, 0.6, 1)
  draw.text(x + 232 - draw.text_width("Echap : fermer"), y + 206, "Echap : fermer", 0.72, 0.74, 0.8, 1)
end)
