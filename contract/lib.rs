#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract]
mod crash_game_casino {
    use ink::storage::Mapping;
    use parity_scale_codec::{Encode, Decode};
    use scale_info::TypeInfo;
    use ink::storage::traits::StorageLayout;

    #[derive(Encode, Decode, Clone, Debug, Default, PartialEq, Eq, TypeInfo, StorageLayout)]
    pub struct Player {
        token_balance: Balance,
        exited: bool,
    }

    #[derive(Encode, Decode, Clone, Debug, Default, PartialEq, Eq, TypeInfo, StorageLayout)]
    pub struct Game {
        id: u64,
        start_block: u32,
        price: Balance,
        crashed: bool,
        game_pool: Balance,
    }

    #[ink(storage)]
    pub struct CrashCasino {
        owner: AccountId,
        game_interval: u32,
        last_game_block: u32,
        current_game_id: u64,
        casino_pool: Balance,
        games: Mapping<u64, Game>,
        players: Mapping<(u64, AccountId), Player>,
    }

    impl CrashCasino {
        #[ink(constructor)]
        pub fn new(game_interval: u32) -> Self {
            let owner = Self::env().caller();
            let block = Self::env().block_number();
            Self {
                owner,
                game_interval,
                last_game_block: block,
                current_game_id: 0,
                casino_pool: 0,
                games: Mapping::default(),
                players: Mapping::default(),
            }
        }

        fn only_owner(&self) {
            assert_eq!(self.env().caller(), self.owner, "Not contract owner");
        }

        fn pseudo_random(&self, salt: &[u8]) -> u8 {
            let entropy = self.env().hash_bytes::<ink::env::hash::Blake2x256>(salt);
            entropy.as_ref()[0]
        }

        #[ink(message)]
        pub fn tick(&mut self) {
            let current_block = self.env().block_number();
            if current_block >= self.last_game_block + self.game_interval {
                self.end_previous_game_if_active();
                self.start_new_game();
            }
        }

        fn start_new_game(&mut self) {
            let current_block = self.env().block_number();
            let game_id = self.current_game_id + 1;
            let new_game = Game {
                id: game_id,
                start_block: current_block,
                price: 1_000_000_000_000,
                crashed: false,
                game_pool: 0,
            };
            self.games.insert(game_id, &new_game);
            self.current_game_id = game_id;
            self.last_game_block = current_block;
        }

        fn end_previous_game_if_active(&mut self) {
            if self.current_game_id == 0 {
                return;
            }
            let mut game = self.games.get(self.current_game_id).unwrap();
            if game.crashed {
                return;
            }
            let salt = [
                self.current_game_id.to_be_bytes().as_ref(),
                &self.env().block_number().to_be_bytes(),
                self.env().caller().as_ref(),
            ]
                .concat();
            let chance = self.pseudo_random(&salt) % 2;
            if chance == 0 {
                game.crashed = true;
                self.games.insert(self.current_game_id, &game);
            }
        }

        #[ink(message, payable)]
        pub fn enter_game(&mut self) {
            let game_id = self.current_game_id;
            let mut game = self.games.get(game_id).expect("No active game");
            assert!(!game.crashed, "Game already crashed");

            let caller = self.env().caller();
            let amount = self.env().transferred_value();
            assert!(amount > 0, "No funds sent");

            let tokens = amount * 1_000_000_000_000 / game.price;
            let key = (game_id, caller);
            let mut player = self.players.get(key).unwrap_or_default();
            player.token_balance += tokens;
            player.exited = false;
            self.players.insert(key, &player);

            game.game_pool += amount;
            self.games.insert(game_id, &game);
            self.casino_pool += amount;
        }

        #[ink(message)]
        pub fn exit_game(&mut self) {
            let game_id = self.current_game_id;
            let mut game = self.games.get(game_id).expect("No active game");
            let caller = self.env().caller();
            let key = (game_id, caller);
            let mut player = self.players.get(key).expect("Not in game");
            assert!(!player.exited, "Already exited");
            assert!(!game.crashed, "Game crashed, too late!");

            let payout = player.token_balance * game.price / 1_000_000_000_000;
            assert!(self.casino_pool >= payout, "Casino has insufficient funds");

            self.env().transfer(caller, payout).expect("Transfer failed");
            self.casino_pool -= payout;
            player.exited = true;
            self.players.insert(key, &player);
        }

        #[ink(message)]
        pub fn set_game_interval(&mut self, new_interval: u32) {
            self.only_owner();
            self.game_interval = new_interval;
        }

        #[ink(message)]
        pub fn get_current_game(&self) -> Option<Game> {
            self.games.get(self.current_game_id)
        }

        #[ink(message)]
        pub fn get_my_status(&self) -> Option<Player> {
            let key = (self.current_game_id, self.env().caller());
            self.players.get(key)
        }

        #[ink(message)]
        pub fn get_casino_pool(&self) -> Balance {
            self.casino_pool
        }

        #[ink(message)]
        pub fn get_block(&self) -> u32 {
            self.env().block_number()
        }
    }
}
