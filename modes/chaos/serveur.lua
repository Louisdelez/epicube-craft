-- Chaos : une arene, un equipement de depart, et toutes les quinze secondes un evenement qui tombe sur
-- tout le monde : armes, echange de places, explosion, soin, coup de froid... Le dernier en vie gagne.
local EVENTS = {
  { "Pluie d'armes : chacun recoit une arme.", function(g)
    local arms = { "katana_en_fer", "hache_ono_en_pierre", "naginata", "dague_tanto", "massue_tetsubo", "lance_yari" }
    for _, p in ipairs(g:players()) do p:give(arms[g:roll(#arms)], 1) end
  end },
  { "Echange : tout le monde change de place !", function(g)
    local list, at = g:players(), {}
    for i, p in ipairs(list) do at[i] = { p:pos() } end
    for i, p in ipairs(list) do
      local to = at[i % #list + 1]
      if to[1] then p:teleport(to[1], to[2], to[3]) end
    end
  end },
  { "Explosion au centre !", function(g) world.explode(g.ox + 0.5, g.oy + 2, g.oz + 0.5, 3) end },
  { "Soin : tout le monde retrouve sa vie.", function(g) for _, p in ipairs(g:players()) do p:set_health(20) end end },
  { "Coup de froid : deux coeurs en moins.", function(g) for _, p in ipairs(g:players()) do p:damage(4) end end },
  { "Archers : un arc et des fleches pour tous.", function(g)
    for _, p in ipairs(g:players()) do p:give("arc_yumi", 1) p:give("fleche", 6) end
  end },
  { "Cuirasses : une armure de fer pour tous.", function(g) for _, p in ipairs(g:players()) do p:give("cuirasse_do_de_fer", 1) end end },
  { "Festin : de quoi manger.", function(g) for _, p in ipairs(g:players()) do p:give("viande_cuite", 3) end end },
}

minigame{ id = "chaos", name = "Chaos", min = 2,
  build = function(g)
    world.fill(g.ox - 12, g.oy, g.oz - 12, g.ox + 12, g.oy, g.oz + 12, "tuiles_kawara")
    for _, w in ipairs({ { -12, -12, 12, -12 }, { -12, 12, 12, 12 }, { -12, -12, -12, 12 }, { 12, -12, 12, 12 } }) do
      world.fill(g.ox + w[1], g.oy + 1, g.oz + w[2], g.ox + w[3], g.oy + 4, g.oz + w[4], "obsidienne")
    end
  end,
  spawn = function(g, p, i)
    local x, z = g:ring(i, 9)
    p:teleport(x + 0.5, g.oy + 1.2, z + 0.5)
    p:give("bokken", 1)
    p:give("onigiri", 3)
  end,
  second = function(g, clock)
    if clock % 15 ~= 0 then return end
    local e = EVENTS[g:roll(#EVENTS)]
    announce(e[1])
    e[2](g)
  end,
  status = function(g) return g.count .. " en vie - evenement dans " .. (15 - g.clock % 15) .. " s" end }
