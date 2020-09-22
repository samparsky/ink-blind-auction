# ink-blind-auction

Blind auction implemented using Substrate INK smart contract programming language.

It implements a basic blind auction name registration system in with Substrate ink! dsl.

# How it works

- User submits in plain text the name they want to register and an auction process starts immediately
- User submits a hash of the name with salt they want to register and a value via `new_bid` function. This prevents anyone from knowing which name the user is bidding 
- After the auction period has ended any user can reveal their bid for the `reveal_period_duration` period of time configured
- After the reveal period has ended, the auction winner can call `finalize_auction` to claim their name
- If the user wants to renew for the configured `expiration_duration` he can call `renew` and it get's renewed

# TODO
- Complete tests
- Add possibility to move from `finalize_auction` to `start_auction` if no one bids for a submitted name

# License

MIT
