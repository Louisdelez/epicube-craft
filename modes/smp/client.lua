-- SMP, cote ecran : la fenetre de l'hotel des ventes.
local W, H = 300, 226
local open, note = false, ""
local page, pages, money, total = 1, 1, 0, 0
local rows = {}             -- { id, objet, nombre, prix, vendeur, a soi }

local function row(i)
  local w, h = draw.screen()
  return (w - W) / 2 + 8, (h - H) / 2 + 24 + (i - 1) * 21, W - 16, 19
end

local function arrows()
  local w, h = draw.screen()
  return (w - W) / 2 + 8, (h - H) / 2 + H - 34
end

on("message", function(name, data)
  if name == "ah" then
    rows = {}
    local first = true
    for line in string.gmatch(data, "[^\n]+") do
      if first then
        local a, b, c, d = line:match("^(%d+)/(%d+)/(%d+)/(%d+)$")
        page, pages, money, total = tonumber(a) or 1, tonumber(b) or 1, tonumber(c) or 0, tonumber(d) or 0
        first = false
      else
        local id, item, n, price, seller, mine = line:match("^(%d+)|([^|]+)|(%d+)|(%d+)|([^|]+)|(%d)$")
        if id then rows[#rows + 1] = { tonumber(id), item, tonumber(n), tonumber(price), seller, mine == "1" } end
      end
    end
    if not open then
      open, note = true, ""
      screen.open()
    end
  elseif name == "ah_note" then
    note = data
  end
end)

on("close", function() open = false end)

on("click", function(x, y, button)
  if not open or button ~= 0 then return end
  for i, r in ipairs(rows) do
    local rx, ry, rw, rh = row(i)
    if x >= rx and x < rx + rw and y >= ry and y < ry + rh then send("ah_acheter", r[1] .. "/" .. page) end
  end
  local ax, ay = arrows()
  if y >= ay and y < ay + 14 then
    if x >= ax and x < ax + 60 and page > 1 then send("ah_page", page - 1) end
    if x >= ax + 64 and x < ax + 124 and page < pages then send("ah_page", page + 1) end
  end
end)

on("hud", function()
  if not open then return end
  local w, h = draw.screen()
  local x, y = (w - W) / 2, (h - H) / 2
  draw.rect(x - 1, y - 1, W + 2, H + 2, 0.3, 0.2, 0.05, 1)
  draw.rect(x, y, W, H, 0.09, 0.08, 0.12, 0.96)
  draw.text(x + 8, y + 8, "Hotel des ventes", 1, 0.86, 0.36, 1)
  local purse = money .. " kobans"
  draw.text(x + W - 8 - draw.text_width(purse), y + 8, purse, 0.6, 1, 0.6, 1)
  local mx, my = screen.mouse()
  if #rows == 0 then draw.text(x + 8, y + 30, "Rien en vente. /ah vendre <prix> met en vente la pile que vous tenez.", 0.72, 0.74, 0.8, 1) end
  for i, r in ipairs(rows) do
    local rx, ry, rw, rh = row(i)
    local hot = mx >= rx and mx < rx + rw and my >= ry and my < ry + rh
    if hot then draw.rect(rx, ry, rw, rh, 0.85, 0.65, 0.16, 0.9) else draw.rect(rx, ry, rw, rh, 0.2, 0.18, 0.26, 0.9) end
    draw.icon(rx + 3, ry + 2, 15, r[2])
    draw.text(rx + 23, ry + 6, r[3] .. " x " .. string.gsub(r[2], "_", " "))
    local right = r[6] and "retirer" or (r[4] .. " kobans - " .. r[5])
    draw.text(rx + rw - draw.text_width(right) - 6, ry + 6, right, 0.8, 0.82, 0.9, 1)
  end
  local ax, ay = arrows()
  draw.rect(ax, ay, 60, 14, 0.2, 0.18, 0.26, 0.9)
  draw.text(ax + 8, ay + 3, "Precedent")
  draw.rect(ax + 64, ay, 60, 14, 0.2, 0.18, 0.26, 0.9)
  draw.text(ax + 76, ay + 3, "Suivant")
  draw.text(ax + 132, ay + 3, "page " .. page .. " / " .. pages .. " - " .. total .. " vente(s)", 0.72, 0.74, 0.8, 1)
  draw.text(x + 8, y + H - 14, note, 0.6, 1, 0.6, 1)
  draw.text(x + W - 8 - draw.text_width("Echap : fermer"), y + H - 14, "Echap : fermer", 0.72, 0.74, 0.8, 1)
end)
