#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract(version = "0.1.0")]
mod registrar {
    use ink_core::storage2::collections::HashMap;
    use ink_core::hash::Keccak256;
    use ink_core::env::block_timestamp;
    use ink_core::env::DefaultEnvTypes;

    #[derive(Debug, PartialEq, Eq, Hash, scale::Encode, scale::Decode)]
    pub enum NameMode {
        Auction,
        Owned,
        Expired,
    }

    #[ink(event)]
    struct AuctionStarted {
        #[ink(topic)]
        name: Hash,
        #[ink(topic)]
        from: AccountId,
    }

    #[ink(event)]
    struct AuctionFinalized {
        #[ink(topic)]
        name: Hash,
        #[ink(topic)]
        from: AccountId,
    }

    #[ink(event)]
    struct NewBid {
        #[ink(topic)]
        sealed_bid: Hash,
        #[ink(topic)]
        from: AccountId,
    }

    #[ink(event)]
    struct CancelBid {
        #[ink(topic)]
        name: Hash,
        #[ink(topic)]
        from: AccountId,
    }

    #[ink(event)]
    struct RevealBid {
        #[ink(topic)]
        name: Hash,
        #[ink(topic)]
        from: AccountId,
    }

    #[ink(event)]
    struct RenewBid {
        #[ink(topic)]
        name: Hash,
        #[ink(topic)]
        from: AccountId,
    }

    pub type RegistrationTimestamp = u64;
    pub type Mode = u64;

    pub const AUCTION_MODE: Mode = 0;
    pub const REVEAL_MODE: Mode = 1;
    pub const OWNED_MODE: Mode = 2;
    pub const EXPIRED_MODE: Mode = 3;

    /// Registrar
    #[ink(storage)]
    #[derive(Default)]
    struct Registrar {
        reveal_period_duration: Timestamp,
        expiration_duration: Timestamp,
        auction_duration: Timestamp,
        min_price: Balance,
        entries: HashMap<Hash, (RegistrationTimestamp, Balance, AccountId, Mode)>,
        sealed_bids: HashMap<(Hash, AccountId), Balance>,
    }

    /// Errors that can occur upon calling this contract.
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(::scale_info::TypeInfo))]
    pub enum Error {
        /// Returned if the name already exists upon registration.
        AuctionStartedAlready,
        /// Returned if caller is not owner while required to.
        InvalidBidPrice,
        InvalidSealedBidHash,
        AuctionInProgress,
        AuctionHasEnded,
        RevealPeriodHasEnded,
        BidNonExistent,
        InvalidNameHash,
        InvalidBidReveal,
        InvalidRenew,
    }

    pub type Result<T> = core::result::Result<T, Error>;


    impl Registrar {

        #[ink(constructor)]
        fn new(reveal_period_duration: u64, auction_duration: u64, expiration_duration: u64, min_price: u128) -> Self {
            Self {
                reveal_period_duration,
                auction_duration,
                expiration_duration,
                min_price,
                entries: Default::default(),
                sealed_bids: Default::default(),
            }
        }

        #[ink(constructor)]
        fn default() -> Self {
            Self::new(
                300, // 5 min
                300, // 5 min
                300, // 5 min
                1 // 1 coin
            )
        }

        #[ink(message)]
        fn start_auction(&mut self, name: Hash) -> Result<()> {
            let caller = self.env().caller();

            match self.entries.get(&name) {
                Some(_) => return Err(Error::AuctionStartedAlready),
                None => {
                    let registration_date = block_timestamp::<DefaultEnvTypes>().unwrap();
                    let highest_bid_amount = self.min_price;
                    self.entries.insert(name, (registration_date, highest_bid_amount, AccountId::from([0; 32]), AUCTION_MODE));
                    self.env().emit_event(AuctionStarted {
                        name,
                        from: caller,
                    });
                    return Ok(());
                }
            }
        }

        #[ink(message)]
        fn new_bid(&mut self, sealed_bid: Hash, value: Balance) -> Result<()> {
            let caller = self.env().caller();
            if value < self.min_price {
                return Err(Error::InvalidBidPrice);
            }

            self.sealed_bids.insert((sealed_bid, caller), value);
            self.env().emit_event(NewBid {
                sealed_bid,
                from: caller,
            });
            return Ok(())
        }

        #[ink(message)]
        fn reveal_bid(&mut self, name: Hash, salt: Hash) -> Result<()> {
            let (registration_date, highest_bid_amount, _, mode) = match self.entries.get(&name).copied() {
                Some((registration_date, highest_bid_amount, account, mode)) => (registration_date, highest_bid_amount, account, mode),
                None => return Err(Error::InvalidNameHash)
            };

            if mode != AUCTION_MODE {
                return Err(Error::AuctionHasEnded);
            }

            let current_block_timestamp = block_timestamp::<DefaultEnvTypes>().unwrap();

            if current_block_timestamp < registration_date + self.auction_duration {
                return Err(Error::AuctionInProgress);
            }

            if current_block_timestamp > registration_date + self.auction_duration + self.reveal_period_duration {
                return Err(Error::RevealPeriodHasEnded);
            }

            let caller = self.env().caller();
            let mut sealed_bid = [0x00_u8; 32];
            let input: Vec<u8> = [name.as_ref(), salt.as_ref()].concat();
            let mut hasher = Keccak256::from(Vec::new());
            hasher.hash_encoded_using(&input, &mut sealed_bid);

            let bid_amount = match self.sealed_bids.get(&(sealed_bid.into(), caller)) {
                Some(amount) => amount,
                None => return Err(Error::InvalidBidReveal)
            };

            if bid_amount > &highest_bid_amount {
                // replace the current entry owner
                // set entry to reveal mode
                self.entries.insert(name, (registration_date, *bid_amount, caller, REVEAL_MODE));
            }

            self.env().emit_event(RevealBid {
                name,
                from: caller,
            });

            return Ok(())
        }

        #[ink(message)]
        fn cancel_bid(&mut self, sealed_bid: Hash) -> Result<()> {
            let caller = self.env().caller();
            let bid = self.sealed_bids.get(&(sealed_bid, caller));
            match bid {
                Some(_) => self.sealed_bids.insert((sealed_bid, caller), 0),
                None => return Err(Error::BidNonExistent)
            };
            Ok(())
        }

        #[ink(message)]
        fn renew(&mut self, name: Hash) -> Result<()> {
            let caller = self.env().caller();
            let (registration_timestamp, highest_bid_amount, _, mode) = *self.entries.get(&name).unwrap();
            if mode != OWNED_MODE || mode != EXPIRED_MODE {
                return Err(Error::InvalidRenew)
            }

            let renew_timestamp = registration_timestamp + self.expiration_duration;
            self.entries.insert(name, (renew_timestamp, highest_bid_amount, caller, mode));

            Ok(())
        }

        #[ink(message)]
        fn finalize_auction(&mut self, name: Hash) -> Result<()> {
            let current_block_timestamp = block_timestamp::<DefaultEnvTypes>().unwrap();
            let (registration_timestamp, highest_bid_amount, caller, mode) = *self.entries.get(&name).unwrap();
            if mode != REVEAL_MODE {
                return Err(Error::AuctionInProgress);
            }
            if current_block_timestamp > registration_timestamp + self.auction_duration + self.reveal_period_duration {
                self.entries.insert(name, (registration_timestamp, highest_bid_amount, caller, OWNED_MODE));
            } else {
                return Err(Error::AuctionInProgress);
            }

            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {

        use super::*;

        use ink_core::env;

        /// Executes the given test through the off-chain environment.
        fn run_test<F>(test_fn: F)
        where
            F: FnOnce(),
        {
            env::test::run_test::<env::DefaultEnvTypes, _>(|_| {
                test_fn();
                Ok(())
            })
            .unwrap()
        }

        #[test]
        fn start_auction() {
            run_test(|| {
                let mut registrar = Registrar::default();
                assert_eq!(registrar.start_auction(Hash::from([1; 32])), Ok(()));
            })
        }

        #[test]
        fn new_bid() {
            run_test(|| {
                let name = [1; 32];
                let mut registrar = Registrar::default();
                assert_eq!(registrar.start_auction(Hash::from(name)), Ok(()));

                let mut sealed_bid = [0x00_u8; 32];

                let input: Vec<u8> = [name, [2; 32]].concat();
                let mut hasher = Keccak256::from(Vec::new());
                hasher.hash_encoded_using(&input, &mut sealed_bid);
                
                assert_eq!(registrar.new_bid(Hash::from(sealed_bid), 1), Ok(()));
            })
        }
    }
}
