//! Commerce avec les villageois. La monnaie est le koban, piece d'or ovale : les villageois en
//! donnent contre les produits de la terre et de la mine, et vendent contre lui ce que leur metier fabrique.
use crate::blocks::*;
use crate::items::*;

pub const FARMER: u8 = 0;
pub const FISHER: u8 = 1;
pub const SMITH: u8 = 2;
pub const PRIEST: u8 = 3;
pub const MERCHANT: u8 = 4;

pub fn profession_name(p: u8) -> &'static str {
    match p % 5 {
        FARMER => "Paysan",
        FISHER => "Pêcheur",
        SMITH => "Forgeron",
        PRIEST => "Prêtre kannushi",
        _ => "Marchand",
    }
}

/// Une offre : ce que le villageois demande, ce qu'il donne.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Offer {
    pub cost: Stack,
    pub out: Stack,
}

/// Les six offres d'un metier : les trois premieres achetent, les trois suivantes vendent.
pub fn offers(profession: u8) -> Vec<Offer> {
    let o = |cost: Id, n: u8, out: Id, k: u8| Offer { cost: Stack::new(cost, n), out: Stack::new(out, k) };
    match profession % 5 {
        FARMER => vec![
            o(WHEAT_ITEM, 20, KOBAN, 1),
            o(CARROT_ITEM, 22, KOBAN, 1),
            o(KABOCHA as Id, 6, KOBAN, 1),
            o(KOBAN, 1, BREAD, 6),
            o(KOBAN, 1, KABOCHA_SEEDS, 4),
            o(KOBAN, 3, BENTO, 1),
        ],
        FISHER => vec![
            o(STRING, 20, KOBAN, 1),
            o(RAW_FISH, 15, KOBAN, 1),
            o(COAL, 10, KOBAN, 1),
            o(KOBAN, 1, COOKED_FISH, 6),
            o(KOBAN, 1, NORI, 4),
            o(KOBAN, 3, FISHING_ROD, 1),
        ],
        SMITH => vec![
            o(COAL, 15, KOBAN, 1),
            o(IRON_INGOT, 4, KOBAN, 1),
            o(DIAMOND, 1, KOBAN, 1),
            o(KOBAN, 3, tool_id(PICK, 2), 1),
            o(KOBAN, 6, tool_id(SWORD, 2), 1),
            o(KOBAN, 5, SHIELD, 1),
        ],
        PRIEST => vec![
            o(ROTTEN, 32, KOBAN, 1),
            o(GOLD_INGOT, 3, KOBAN, 1),
            o(BONE, 24, KOBAN, 1),
            o(KOBAN, 1, VOLT_DUST, 2),
            o(KOBAN, 2, DRIED_TEA, 3),
            o(KOBAN, 5, PEARL, 1),
        ],
        _ => vec![
            o(STICK, 32, KOBAN, 1),
            o(FEATHER, 24, KOBAN, 1),
            o(LEATHER, 6, KOBAN, 1),
            o(KOBAN, 1, ARROW, 16),
            o(KOBAN, 2, KUNAI, 4),
            o(KOBAN, 6, YUMI, 1),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_profession_buys_and_sells() {
        for p in 0..5 {
            let list = offers(p);
            assert_eq!(list.len(), 6);
            assert!(list[..3].iter().all(|o| o.out.id == KOBAN && o.cost.id != KOBAN), "{} achete", profession_name(p));
            assert!(list[3..].iter().all(|o| o.cost.id == KOBAN && o.out.id != KOBAN), "{} vend", profession_name(p));
            for o in &list {
                assert!(o.cost.n <= max_stack(o.cost.id) && o.out.n <= max_stack(o.out.id));
            }
        }
    }
}
