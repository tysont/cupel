package ledger.cmd;

import ledger.Account;
import ledger.Ledger;

/** Prints a statement for the current user. */
public final class Main {
    private Main() {
    }

    public static void main(String[] args) {
        Account account = Ledger.open(System.getenv("USER"));
        account.deposit(1250);
        System.out.println(account.statement());
    }
}
