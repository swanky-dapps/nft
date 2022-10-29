#![cfg_attr(not(feature = "std"), no_std)]
#![feature(min_specialization)]

#[openbrush::contract]
pub mod shiden34 {
    // imports from ink!
    use ink_lang::codegen::Env;
    use ink_prelude::string::{
        String,
        ToString,
    };
    use ink_storage::traits::SpreadAllocate;

    // imports from openbrush
    use openbrush::{
        contracts::{
            ownable::*,
            psp34::extensions::{
                burnable::*,
                enumerable::*,
                metadata::*,
                mintable::*,
            },
            reentrancy_guard::*,
        },
        modifiers,
        traits::Storage,
    };

    #[ink(storage)]
    #[derive(Default, SpreadAllocate, Storage)]
    pub struct Shiden34Contract {
        #[storage_field]
        psp34: psp34::Data<enumerable::Balances>,
        #[storage_field]
        metadata: metadata::Data,
        #[storage_field]
        guard: reentrancy_guard::Data,
        #[storage_field]
        ownable: ownable::Data,

        // contract specific
        last_token_id: u64,
        collection_id: u32,
        max_supply: u64,
        price_per_mint: Balance,
    }

    // Section contains default implementation without any modifications
    impl PSP34 for Shiden34Contract {}
    impl PSP34Burnable for Shiden34Contract {}
    impl PSP34Enumerable for Shiden34Contract {}
    impl PSP34Metadata for Shiden34Contract {}
    impl Ownable for Shiden34Contract {}

    #[openbrush::trait_definition]
    pub trait Shiden34Trait {
        #[ink(message, payable)]
        fn mint_for(&mut self, to: AccountId, mint_amount: u64) -> Result<(), PSP34Error>;
        #[ink(message)]
        fn set_base_uri(&mut self, uri: String) -> Result<(), PSP34Error>;
        #[ink(message)]
        fn token_uri(&self, token_id: u64) -> Result<String, PSP34Error>;
        #[ink(message)]
        fn max_supply(&self) -> u64;
    }

    impl Shiden34Contract {
        #[ink(constructor)]
        pub fn new(
            name: String,
            symbol: String,
            base_uri: String,
            max_supply: u64,
            price_per_mint: Balance,
        ) -> Self {
            ink_lang::codegen::initialize_contract(|_instance: &mut Shiden34Contract| {
                _instance._set_attribute(
                    Id::U8(0),
                    String::from("name").into_bytes(),
                    String::from(name).into_bytes(),
                );
                _instance._set_attribute(
                    Id::U8(0),
                    String::from("symbol").into_bytes(),
                    String::from(symbol).into_bytes(),
                );
                _instance._set_attribute(
                    Id::U8(0),
                    String::from("baseUri").into_bytes(),
                    String::from(base_uri).into_bytes(),
                );
                _instance.max_supply = max_supply;
                _instance.price_per_mint = price_per_mint;
                _instance.last_token_id = 0;
                let caller = _instance.env().caller();
                _instance._init_with_owner(caller);
            })
        }

        /// Check id the transfered mint values is as expected
        fn check_value(&self, mint_amount: u64) -> Result<(), PSP34Error> {
            if Self::env().transferred_value() != mint_amount as u128 * self.price_per_mint {
                return Err(PSP34Error::Custom("BadMintValue".to_string()))
            }
            Ok(())
        }

        /// Check amount of tokens to be minted
        fn check_amount(&self, mint_amount: u64) -> Result<(), PSP34Error> {
            if mint_amount == 0 {
                return Err(PSP34Error::Custom("CannotMintZeroTokens".to_string()))
            }

            if self.last_token_id + mint_amount > self.max_supply {
                return Err(PSP34Error::Custom("CollectionFullOrLocked".to_string()))
            }
            Ok(())
        }

        /// Check if token is minted
        fn token_exists(&self, id: Id) -> Result<(), PSP34Error> {
            self.owner_of(id).ok_or(PSP34Error::TokenNotExists)?;
            Ok(())
        }
    }

    impl PSP34Mintable for Shiden34Contract {
        /// Mint next available token for the caller. The arguments account and id are ignored
        #[ink(message)]
        fn mint(&mut self, _: AccountId, _: Id) -> Result<(), PSP34Error> {
            self.check_value(1)?;
            let caller = self.env().caller();
            self.last_token_id += 1;
            self._mint_to(caller, Id::U64(self.last_token_id))?;

            Ok(())
        }
    }

    impl Shiden34Trait for Shiden34Contract {
        /// Mint several tokens
        #[ink(message, payable)]
        #[modifiers(non_reentrant)]
        fn mint_for(&mut self, to: AccountId, mint_amount: u64) -> Result<(), PSP34Error> {
            self.check_value(mint_amount)?;
            self.check_amount(mint_amount)?;

            let next_to_mint = self.last_token_id + 1; // first mint id is 1
            let mint_offset = next_to_mint + mint_amount;

            for mint_id in next_to_mint..mint_offset {
                assert!(self._mint_to(to, Id::U64(mint_id)).is_ok());
                self.last_token_id += 1;
            }

            Ok(())
        }

        /// Set new value for the baseUri
        #[ink(message)]
        #[modifiers(only_owner)]
        fn set_base_uri(&mut self, uri: String) -> Result<(), PSP34Error> {
            self._set_attribute(
                Id::U8(0),
                String::from("baseUri").into_bytes(),
                uri.into_bytes(),
            );
            Ok(())
        }

        /// Get URI from token ID
        #[ink(message)]
        fn token_uri(&self, token_id: u64) -> Result<String, PSP34Error> {
            _ = self.token_exists(Id::U64(token_id))?;
            let value = self.get_attribute(Id::U8(0), String::from("baseUri").into_bytes());
            let mut token_uri = String::from_utf8(value.unwrap()).unwrap();
            token_uri = token_uri + &token_id.to_string() + &String::from(".json");
            Ok(token_uri)
        }

        /// Get max supply of tokens
        #[ink(message)]
        fn max_supply(&self) -> u64 {
            self.max_supply
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use ink_lang as ink;
        const PRICE: Balance = 100_000_000_000_000_000;
        const BASE_URI: &str = "ipfs://myIpfsUri/";
        const MAX_SUPPLY: u64 = 10;
        use crate::shiden34::PSP34Error::*;
        use ink_env::test;

        #[ink::test]
        fn init_works() {
            let sh34 = init();
            assert_eq!(
                sh34.get_attribute(Id::U8(0), String::from("name").into_bytes()),
                Some(String::from("Shiden34").into_bytes())
            );
            assert_eq!(
                sh34.get_attribute(Id::U8(0), String::from("symbol").into_bytes()),
                Some(String::from("SH34").into_bytes())
            );
            assert_eq!(
                sh34.get_attribute(Id::U8(0), String::from("baseUri").into_bytes()),
                Some(String::from(BASE_URI).into_bytes())
            );
            assert_eq!(sh34.max_supply, MAX_SUPPLY);
            assert_eq!(sh34.price_per_mint, PRICE);
        }

        fn init() -> Shiden34Contract {
            Shiden34Contract::new(
                String::from("Shiden34"),
                String::from("SH34"),
                String::from(BASE_URI),
                MAX_SUPPLY,
                PRICE,
            )
        }

        #[ink::test]
        fn mint_single_works() {
            let mut sh34 = init();
            let accounts = default_accounts();
            set_sender(accounts.alice);

            assert_eq!(sh34.total_supply(), 0);
            test::set_value_transferred::<ink_env::DefaultEnvironment>(PRICE);
            assert!(sh34.mint(accounts.alice, Id::U64(0)).is_ok());
            assert_eq!(sh34.total_supply(), 1);
            assert_eq!(sh34.owner_of(Id::U64(1)), Some(accounts.alice));
            assert_eq!(sh34.balance_of(accounts.alice), 1);
            assert_eq!(
                sh34.owners_token_by_index(accounts.alice, 0),
                Ok(Id::U64(1))
            );
            assert_eq!(sh34.last_token_id, 1);
        }

        #[ink::test]
        fn mint_multiple_works() {
            let mut sh34 = init();
            let accounts = default_accounts();
            set_sender(accounts.alice);
            let num_of_mints: u64 = 5;

            assert_eq!(sh34.total_supply(), 0);
            test::set_value_transferred::<ink_env::DefaultEnvironment>(
                PRICE * num_of_mints as u128,
            );
            assert!(sh34.mint_for(accounts.bob, num_of_mints).is_ok());
            assert_eq!(sh34.total_supply(), num_of_mints as u128);
            assert_eq!(sh34.balance_of(accounts.bob), 5);
            assert_eq!(sh34.owners_token_by_index(accounts.bob, 0), Ok(Id::U64(1)));
            assert_eq!(sh34.owners_token_by_index(accounts.bob, 1), Ok(Id::U64(2)));
            assert_eq!(sh34.owners_token_by_index(accounts.bob, 2), Ok(Id::U64(3)));
            assert_eq!(sh34.owners_token_by_index(accounts.bob, 3), Ok(Id::U64(4)));
            assert_eq!(sh34.owners_token_by_index(accounts.bob, 4), Ok(Id::U64(5)));
            assert_eq!(
                sh34.owners_token_by_index(accounts.bob, 5),
                Err(TokenNotExists)
            );
        }

        #[ink::test]
        fn mint_above_limit_fails() {
            let mut sh34 = init();
            let accounts = default_accounts();
            set_sender(accounts.alice);
            let num_of_mints: u64 = MAX_SUPPLY + 1;

            assert_eq!(sh34.total_supply(), 0);
            test::set_value_transferred::<ink_env::DefaultEnvironment>(
                PRICE * num_of_mints as u128,
            );
            assert_eq!(
                sh34.mint_for(accounts.bob, num_of_mints),
                Err(Custom("CollectionFullOrLocked".to_string()))
            );
        }

        #[ink::test]
        fn mint_low_value_fails() {
            let mut sh34 = init();
            let accounts = default_accounts();
            set_sender(accounts.alice);
            let num_of_mints = 1;

            assert_eq!(sh34.total_supply(), 0);
            test::set_value_transferred::<ink_env::DefaultEnvironment>(
                PRICE * num_of_mints as u128 - 1,
            );
            assert_eq!(
                sh34.mint_for(accounts.bob, num_of_mints),
                Err(Custom("BadMintValue".to_string()))
            );
        }

        #[ink::test]
        fn token_uri_works() {
            let mut sh34 = init();
            let accounts = default_accounts();
            set_sender(accounts.alice);

            test::set_value_transferred::<ink_env::DefaultEnvironment>(PRICE);
            assert!(sh34.mint(accounts.alice, Id::U64(0)).is_ok());
            // assert_eq!(
            //     sh34.token_uri(1),
            //     Ok(BASE_URI.to_owned() + &String::from("1.json"))
            // );
            // return error if request is for not yet minted token
            assert_eq!(sh34.token_uri(42), Err(TokenNotExists));
        }

        #[ink::test]
        fn owner_is_set() {
            let accounts = default_accounts();
            let sh34 = init();
            assert_eq!(sh34.owner(), accounts.alice);
        }

        #[ink::test]
        fn set_base_uri_works() {
            let accounts = default_accounts();
            const NEW_BASE_URI: &str = "new_uri/";
            let mut sh34 = init();

            set_sender(accounts.alice);
            assert!(sh34.set_base_uri(NEW_BASE_URI.to_string()).is_ok());
            assert_eq!(
                sh34.get_attribute(Id::U8(0), String::from("baseUri").into_bytes()),
                Some(String::from(NEW_BASE_URI).into_bytes())
            );
            set_sender(accounts.bob);
            assert_eq!(
                sh34.set_base_uri("shallFail".to_string()),
                Err(Custom("O::CallerIsNotOwner".to_string()))
            );
        }

        fn default_accounts() -> test::DefaultAccounts<ink_env::DefaultEnvironment> {
            test::default_accounts::<Environment>()
        }

        fn set_sender(sender: AccountId) {
            ink_env::test::set_caller::<Environment>(sender);
        }
    }
}
