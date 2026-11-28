-- Rush, cote ecran : l'etat des equipes en haut, et la fenetre de la boutique.
local SHOP = {
  { "fourrure", "16 fourrures", "4 fer" },
  { "planches_de_hinoki", "8 planches", "6 fer" },
  { "katana_en_pierre", "Katana en pierre", "10 fer" },
  { "pioche_en_bois", "Pioche en bois", "8 fer" },
  { "onigiri", "4 onigiri", "3 fer" },
  { "katana_en_fer", "Katana en fer", "3 diamants" },
  { "cuirasse_do_de_fer", "Cuirasse de fer", "4 diamants" },
  { "obsidienne", "4 obsidiennes", "4 diamants" },
  { "pierre_de_muraille", "12 pierres", "4 or" },
  { "pioche_en_fer", "Pioche en fer", "6 or" },
  { "arc_yumi", "Arc yumi", "12 or" },
  { "fleche", "8 fleches", "2 or" },
  { "bouclier_tate", "Bouclier", "5 or" },
  { "feux_d_artifice", "Feux d'artifice", "8 or" },
  { "pomme", "2 pommes", "3 or" },
  { "katana_en_diamant", "Katana en diamant", "4 braises" },
  { "casque_kabuto_de_fer", "Casque de fer", "2 braises" },
  { "jambieres_suneate_de_fer", "Jambieres de fer", "2 braises" },
}
-- La fenetre : deux colonnes de neuf articles, et l'amelioration de la forge en bas.
local W, H, ROWS = 452, 250, 9
local state, open, note = "", false, ""

local function row(i)
  local w, h = draw.screen()
  local col = math.floor((i - 1) / ROWS)
  return (w - W) / 2 + 8 + col * 220, (h - H) / 2 + 24 + ((i - 1) % ROWS) * 21, 216, 19
end

local function forge_row()
  local w, h = draw.screen()
  return (w - W) / 2 + 8, (h - H) / 2 + 24 + ROWS * 21 + 2, 216, 19
end

on("message", function(name, data)
  if name == "etat" then state = data
  elseif name == "boutique" then
    open, note = true, ""
    screen.open()
  elseif name == "achat" then
    note = data == "ok" and "Achete !" or data == "deja" and "La forge est deja amelioree." or "Pas assez de quoi payer."
  end
end)

on("close", function() open = false end)

on("click", function(x, y, button)
  if not open or button ~= 0 then return end
  for i = 1, #SHOP do
    local rx, ry, rw, rh = row(i)
    if x >= rx and x < rx + rw and y >= ry and y < ry + rh then send("acheter", i) end
  end
  local fx, fy, fw, fh = forge_row()
  if x >= fx and x < fx + fw and y >= fy and y < fy + fh then send("forge", "") end
end)

on("hud", function()
  local w, h = draw.screen()
  if state ~= "" then
    local tw = draw.text_width(state)
    draw.rect((w - tw) / 2 - 5, 4, tw + 10, 13, 0, 0, 0, 0.55)
    draw.text((w - tw) / 2, 7, state, 1, 0.86, 0.36, 1)
  end
  if not open then return end
  local x, y = (w - W) / 2, (h - H) / 2
  draw.rect(x - 1, y - 1, W + 2, H + 2, 0.3, 0.2, 0.05, 1)
  draw.rect(x, y, W, H, 0.09, 0.08, 0.12, 0.96)
  draw.text(x + 8, y + 8, "Boutique", 1, 0.86, 0.36, 1)
  local mx, my = screen.mouse()
  local function line(rx, ry, rw, rh, icon, label, price)
    local hot = mx >= rx and mx < rx + rw and my >= ry and my < ry + rh
    if hot then draw.rect(rx, ry, rw, rh, 0.85, 0.65, 0.16, 0.9) else draw.rect(rx, ry, rw, rh, 0.2, 0.18, 0.26, 0.9) end
    draw.icon(rx + 3, ry + 2, 15, icon)
    draw.text(rx + 23, ry + 6, label)
    draw.text(rx + rw - draw.text_width(price) - 6, ry + 6, price, 0.8, 0.82, 0.9, 1)
  end
  for i, item in ipairs(SHOP) do
    local rx, ry, rw, rh = row(i)
    line(rx, ry, rw, rh, item[1], item[2], item[3])
  end
  local fx, fy, fw, fh = forge_row()
  line(fx, fy, fw, fh, "lingot_de_fer", "Forge d'equipe x2", "6 diamants")
  draw.text(x + 236, y + H - 14, note, 0.6, 1, 0.6, 1)
  draw.text(x + W - 8 - draw.text_width("Echap : fermer"), y + H - 14, "Echap : fermer", 0.72, 0.74, 0.8, 1)
end)
