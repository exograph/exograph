CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE accounts (
    account_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    balance DECIMAL(15,2) NOT NULL DEFAULT 0
);

-- Multiple relations between the same table (account and counterparty_account)
CREATE TABLE transactions (
    transaction_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    account_id UUID NOT NULL REFERENCES accounts(account_id),
    amount DECIMAL(15,2) NOT NULL,
    counterparty_account_id UUID REFERENCES accounts(account_id)
);
