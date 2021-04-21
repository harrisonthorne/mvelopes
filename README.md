# silverfox

silverfox is a command-line double-entry plain-text
rootin-tootin accounting tool like ledger-cli or hledger.
But unlike other plain-text accounting tools, silverfox is
designed around "envelope budgeting". Its main concern is
helping you budget without overspending as well as keeping a
good record of your finances.

## Foreword

silverfox has matured some and is ready for basic ledger
parsing, balance reporting, and envelope use. Give it a try!

Since silverfox is so new, it lacks a lot of features that
other plain-text accounting tools have, so please feel free
to open new issues as your needs arise. I'm also open to new
ideas and to implement new features that other plain-text
accounting tools don't have. I look forward to hearing your
ideas and requests!

## What sets silverfox apart from other tools?

### Expenses, savings goals, and more

silverfox works by moving money from your available balances
into "envelopes" every day. Little by little (or lots by
lots), it puts money towards anything you might be spending
money on: rent, electricity, or a new boat (whatever; you
name it).

For any recurring expenses, silverfox keeps two envelopes:
one for what's ready to be spent, and another for the next
time an expense is due. This ensures a separation of
concerns from due date to due date.

#### Methods of saving money

silverfox lets you choose from two methods of automatically
moving money into envelopes: aggressive or conservative.
Aggressive saving moves as much money as possible into
envelopes as soon as possible. Conservative saving moves
money in little by little every day, saving up just in time
for a due date.

Of course, you can also move money manually and disable
automated saving on a per-envelope basis.

### Required transaction statuses

Each transaction entry must be entered with one of three
statuses:

```
?       Pending
~       Cleared
*       Reconciled
```

silverfox will let you know if a transaction is not marked
with one of these statuses.

### Required account definitions

silverfox requires you to define a list of accounts. You can
use the `account` property to do this:

```
account assets:checking
account assets:savings
account expenses:groceries
account expenses:auto:gas
account expenses:home:rent
account expenses:home:electricity
account income:salary
account income:gifts
```

Accounts in silverfox can't include spaces in their names.
Instead, underscores should be used:

```
account expenses:food:dining_out
account liabilities:credit_card
```

### Speedy!

silverfox will try to be as fast as possible, which will
hopefully come in handy for big ledgers. Eventually,
silverfox will take advantage of concurrency to process
transactions as fast as possible.

## Journal format

If you've used ledger-cli or hledger, silverfox uses a
similar formatting syntax for journals:

```
2019/08/02 * Groceries
    assets:checking     -30
    expenses:groceries   30
```

More specifically:

```
<date> <status> <description> [<payee>]
    <account> <amount>
    <account> <amount>
```

`<payee>` is not required, but if provided in square
brackets as above, can be queried by silverfox.

> Note: While silverfox has formatting extremely similar to
> hledger or ledger-cli, a silverfox file is not necessarily
> compatible with an hledger or ledger-cli file. You'll
> notice that (for example) an equal sign (=) in a posting
> (an account-amount line in a transaction entry) has two
> very different meanings between hledger and silverfox:
> hledger uses it for balance assertions, and silverfox uses
> it for cost assertions.
>
> Also note: There is a hope to change this incompatibility.

### Comments

Comments in silverfox are done with either a semicolon (`;`)
or a double-slash (`//`).

```
2019/08/02 ? Restaurant [Fancy's]
    assets:checking     -140        // This is a comment
    expenses:dining      140        ; This is also a comment
```

> Note: `silverfox` will prefer double slashes. The `format`
> subcommand (coming soon!) will replace semicolons with
> double slashes.

### Currencies and prices

If a currency symbol isn't included, silverfox considers it
your default currency. You can include currency symbols in
your transaction:

```
2019/08/02 * Bought crypto
    assets:checking     $-100
    assets:crypto:btc       0.012345 BTC
```

Note that in the above transaction, silverfox will
automatically balance and infer that the total cost of
0.012345 BTC (Bitcoin) was \$100. You can define the total
cost of BTC manually as well:

```
2019/08/02 * Bought crypto
    assets:checking     $-100
    assets:crypto:btc       0.012345 BTC = $100
```

Or, you can define the price per unit:

```
2019/08/02 * Bought crypto
    assets:checking     $-100
    assets:crypto:btc       0.012345 BTC @ $8100.45
```

If you mix amounts without a currency symbol and amounts
with your preferred currency symbol, you can tell silverfox
which currency symbol you use:

```
currency $
```

silverfox will combine amounts with blank symbols and with
the specified symbol.

### Balance assertions

```
2019/08/02 * Account closure
    assets:checking -100 ! 0
    assets:new_acct  100
```

Balance assertions are used to make sure the amount in an
account is what you expect it to be. The exclamation mark is
used to set balance assertions. With only one exclamation
mark, the assertion operates on a per-currency basis:

```
2019/08/02 * Crypto account closure
    assets:crypto_wallet    -0.01 BTC ! 0
    assets:crypto_wallet    -1.00 ETH ! 0
    assets:crypto_wallet    -32.0 BAT ! 0
    assets:new_wallet        0.01 BTC
    assets:new_wallet        1.00 ETH
    assets:new_wallet        32.0 BAT
```

## Envelopes

### Configuration

Here's an example of how to configure envelopes with
silverfox. Envelopes are created under asset accounts:

```
account assets:checking
    expense rent due every 15th                 // due the 15th of every month
        amount 1000                             // for $1000
        for expenses:home:rent                  // automatically moves money when expenses:home:rent is used
        funding aggressive                      // use aggressive funding

    expense food due every 1st                  // due the 1st of every month
        amount 300                              // for $300
        for expenses:groceries                  // automatically moves money when expenses:groceries is used
        for expenses:dining                     // and expenses:dining

    expense some_weekly_thing due every Monday  // due weekly on Monday (could also write `Mon`)
        amount 100                              // for $300
                                                // accounts aren't required
```

> Note: the `funding` option is optional. If omitted,
> however, silverfox won't move money automatically. 

If you want to delay the starting date for an envelope, you
can do so with `starting`:

```
account assets:checking
    expense streaming due every 20th starting 2019/10/20    // due the 20th of every month, only after the free trial tho
        amount 12                                           // for $12
        for expenses:entertainment                          // automatically moves money when expenses:entertainment is used
```

If you opt for the frequency of an envelope to be `every
other` something, `starting` is required (so silverfox can
know which weeks or months to use):

```
account assets:checking
    expense electricity due every other 15th starting 2020/01/20    // due the 15th of every other month, but not until the new electricity bill starts
        amount 100                                                  // for $100
        for expenses:home:electricity                               // automatically moves money when expenses:home:electricity is used
        funding conservative                                        // use conservative funding
```

We can also do envelopes for goals:

```
account assets:savings
    goal camera by 2020/12/25                       // "due" and "by" are interchangeable
        amount 1000                                 // you get the gist!
        funding conservative

    goal new_boat by 2025/06/28
        amount 35000
        funding conservative
```

Really, the `expense` and `goal` keywords are both
interchangeable. They do the same thing. And, as mentioned,
`due`, `by`, and even `due by` are the same. But not `by
due`. That makes no sense.

A couple of other points to note:

- Expenses and goals can co-exist under the same account
- Like account names, envelope names can't have spaces; use
  underscores instead

### Manual envelope movements

```
2019/08/02 * Groceries
    assets:checking         -50
    expenses:groceries       50
    envelope food
```

silverfox infers that this transaction should take money from
the `food` envelope under the `assets:checking` account.

Of course, things can get a little more complicated:

```
2019/08/02 * Groceries with cash back
    assets:checking                -70
    assets:cash                     20
    expenses:groceries              50
    envelope food assets:checking  -50
```

If `assets:checking` and `assets:cash` both have `food`
envelopes, silverfox can't infer which account from which to
use an envelope. We tell silverfox which envelope to use (and
how much money) with this syntax:

```
envelope <account> <envelope_name> <amount>
```

## Fun facts

(this was originally going to work alongside hledger but then
I decided to make a full-blown accounting program because I
decided I didn't like Haskell and I wanted to learn Rust)

So, this project features my first attempts at Rust. I'm
learning as I go. Good experience so far :)

## Donate

Asking for money isn't my thing, but if you're interested in
helping to fund my education, rent, food, or even just a hot
chocolate, you're welcome to send me Bitcoin or Ethereum:

BTC

```
35yDrjaUwdFgLfJjboYxcY1mNLm1EvMLys
```

ETH

```
0x05d639861B8B7058ae237B41ef71ca2291A295e3
```

Thank you! Your interest in this project means the world for
me.
