"""Builds the Jev A/B project: a Python billing package, 14 modules, one planted bug.

The bug: proration truncates cents with int() inside subscriptions.py `_prorated_amount`, while the
module named proration.py only counts days (a decoy). tests/test_proration.py fails 1666 != 1667.
Usage: python make_repo.py <dir>
"""
import subprocess
import sys
import textwrap
from pathlib import Path

root = Path(sys.argv[1])
SCALE = int(sys.argv[2]) if len(sys.argv) > 2 else 1
if root.exists():
    raise SystemExit(f"{root} exists")
pkg = root / "ledgerly"
tests = root / "tests"
pkg.mkdir(parents=True)
tests.mkdir()

NOUNS = {
    "customers": ["customer", "contact", "address", "account_owner"],
    "plans": ["plan", "price_tier", "feature_flag", "trial"],
    "invoices": ["invoice", "line_item", "credit_note", "payment_term"],
    "taxes": ["tax_rate", "jurisdiction", "exemption", "tax_line"],
    "discounts": ["coupon", "promotion", "volume_break", "referral"],
    "usage": ["meter", "usage_record", "aggregation", "billing_period"],
    "reports": ["revenue_row", "mrr_snapshot", "churn_cohort", "aging_bucket"],
    "notifications": ["email", "dunning_step", "receipt", "reminder"],
    "storage": ["record", "index", "migration", "snapshot"],
    "webhooks": ["event", "endpoint", "signature", "retry_policy"],
}


def filler(module: str) -> str:
    out = [f'"""ledgerly.{module}: {module.replace("_", " ")} for the billing service.\n\nEvery amount here is integer cents.\nRounding of fractional cents happens in ledgerly.money, never here.\n"""', "from __future__ import annotations", "", "from dataclasses import dataclass, field", "from datetime import date, timedelta", ""]
    for noun in NOUNS[module]:
        cls = "".join(w.capitalize() for w in noun.split("_"))
        out.append(textwrap.dedent(f'''
        @dataclass
        class {cls}:
            """A {noun.replace("_", " ")} as stored by ledgerly.storage."""

            id: str
            account_id: str
            created: date
            amount_cents: int = 0
            currency: str = "USD"
            metadata: dict = field(default_factory=dict)

            def is_active(self, on: date) -> bool:
                """Whether the {noun.replace("_", " ")} applies on the given day."""
                return self.created <= on and not self.metadata.get("archived", False)

            def describe(self) -> str:
                return f"{cls} {{self.id}} for {{self.account_id}}: {{self.amount_cents}} {{self.currency}}"


        def find_{noun}s(items: list[{cls}], account_id: str) -> list[{cls}]:
            """Every {noun.replace("_", " ")} of one account, oldest first."""
            return sorted((i for i in items if i.account_id == account_id), key=lambda i: i.created)


        def total_{noun}_cents(items: list[{cls}], since: date | None = None) -> int:
            """The sum of the amounts, in cents, optionally only those created since a day."""
            total = 0
            for item in items:
                if since is not None and item.created < since:
                    continue
                total += item.amount_cents
            return total


        def validate_{noun}(item: {cls}) -> list[str]:
            """Problems with a {noun.replace("_", " ")}, as messages; empty when it is valid."""
            problems = []
            if not item.id:
                problems.append("missing id")
            if item.amount_cents < 0:
                problems.append("negative amount")
            if len(item.currency) != 3:
                problems.append("currency must be an ISO code")
            if item.created > date.today() + timedelta(days=1):
                problems.append("created in the future")
            return problems
        '''))
    return "\n".join(out) + "\n"


EXTRA = ["ledger", "batch", "audit", "rule", "schedule", "export", "cache", "policy", "window", "bucket"]
if SCALE > 1:
    for module in list(NOUNS):
        NOUNS[module] = NOUNS[module] + [f"{w}_{n}" for n in NOUNS[module] for w in EXTRA[: SCALE - 1]]
for module in NOUNS:
    (pkg / f"{module}.py").write_text(filler(module), encoding="utf-8", newline="\n")

(pkg / "__init__.py").write_text('"""ledgerly: a small subscription billing service."""\n', encoding="utf-8", newline="\n")
(pkg / "money.py").write_text(textwrap.dedent('''\
    """ledgerly.money: integer cents and the one rounding rule.

    Fractional cents are rounded half up, always through round_cents. Never truncate with int().
    """
    from __future__ import annotations

    from decimal import ROUND_HALF_UP, Decimal


    def round_cents(value: float | Decimal) -> int:
        """Fractional cents to whole cents, half up: 1666.67 -> 1667, 1666.5 -> 1667."""
        return int(Decimal(str(value)).quantize(Decimal("1"), rounding=ROUND_HALF_UP))


    def split_evenly(total_cents: int, parts: int) -> list[int]:
        """Split cents into parts that differ by at most one cent and sum to the total."""
        base, extra = divmod(total_cents, parts)
        return [base + (1 if i < extra else 0) for i in range(parts)]


    def format_cents(cents: int, currency: str = "USD") -> str:
        return f"{cents / 100:,.2f} {currency}"
    '''), encoding="utf-8", newline="\n")
(pkg / "proration.py").write_text(textwrap.dedent('''\
    """ledgerly.proration: how many days of a period a change covers.

    Only days are counted here. The amount is computed by ledgerly.subscriptions.
    """
    from __future__ import annotations

    from datetime import date


    def days_in_period(start: date, end: date) -> int:
        """Days from start to end, end excluded."""
        return (end - start).days


    def days_remaining(change_on: date, end: date) -> int:
        """Days a change made on change_on covers, up to the period's end."""
        return max(0, (end - change_on).days)


    def fraction_remaining(change_on: date, start: date, end: date) -> float:
        total = days_in_period(start, end)
        return days_remaining(change_on, end) / total if total else 0.0
    '''), encoding="utf-8", newline="\n")
(pkg / "subscriptions.py").write_text(textwrap.dedent('''\
    """ledgerly.subscriptions: plan changes in the middle of a billing period."""
    from __future__ import annotations

    from dataclasses import dataclass
    from datetime import date

    from ledgerly import money, proration


    @dataclass
    class Subscription:
        id: str
        account_id: str
        plan_price_cents: int
        period_start: date
        period_end: date


    def _prorated_amount(price_cents: int, change_on: date, start: date, end: date) -> int:
        """The part of a period's price a mid-period change covers, in cents."""
        fraction = proration.fraction_remaining(change_on, start, end)
        return int(price_cents * fraction)


    def upgrade_charge(sub: Subscription, new_price_cents: int, change_on: date) -> int:
        """What an upgrade on change_on costs for the rest of the period: new minus old, prorated."""
        new = _prorated_amount(new_price_cents, change_on, sub.period_start, sub.period_end)
        old = _prorated_amount(sub.plan_price_cents, change_on, sub.period_start, sub.period_end)
        return new - old


    def cancel_refund(sub: Subscription, cancel_on: date) -> int:
        """What a cancellation on cancel_on refunds, in cents."""
        return _prorated_amount(sub.plan_price_cents, cancel_on, sub.period_start, sub.period_end)


    def renew(sub: Subscription, next_end: date) -> Subscription:
        return Subscription(sub.id, sub.account_id, sub.plan_price_cents, sub.period_end, next_end)


    def monthly_equivalent(price_cents: int, months: int) -> list[int]:
        return money.split_evenly(price_cents, months)
    '''), encoding="utf-8", newline="\n")
if SCALE > 1:
    NOUNS["subscriptions_extra"] = [f"{w}_plan_change" for w in EXTRA[: SCALE - 1]] + [f"{w}_renewal" for w in EXTRA[: SCALE - 1]]
    extra = filler("subscriptions_extra").split("\n", 6)[-1]
    with open(pkg / "subscriptions.py", "a", encoding="utf-8", newline="\n") as fh:
        fh.write("\n\n# ---- records kept beside subscriptions ----\n" + extra)
(tests / "__init__.py").write_text("", encoding="utf-8")
(tests / "test_money.py").write_text(textwrap.dedent('''\
    import unittest

    from ledgerly import money


    class MoneyTest(unittest.TestCase):
        def test_round_half_up(self):
            self.assertEqual(money.round_cents(1666.67), 1667)
            self.assertEqual(money.round_cents(1666.5), 1667)

        def test_split(self):
            self.assertEqual(money.split_evenly(1000, 3), [334, 333, 333])
    '''), encoding="utf-8", newline="\n")
(tests / "test_proration.py").write_text(textwrap.dedent('''\
    import unittest
    from datetime import date

    from ledgerly.subscriptions import Subscription, cancel_refund


    class ProrationTest(unittest.TestCase):
        def test_cancel_refund_rounds_half_up(self):
            sub = Subscription("s1", "a1", 5000, date(2026, 9, 1), date(2026, 10, 1))
            # 10 of 30 days left: 5000 * 10 / 30 = 1666.67 cents, which rounds to 1667.
            self.assertEqual(cancel_refund(sub, date(2026, 9, 21)), 1667)

        def test_full_period(self):
            sub = Subscription("s1", "a1", 5000, date(2026, 9, 1), date(2026, 10, 1))
            self.assertEqual(cancel_refund(sub, date(2026, 9, 1)), 5000)
    '''), encoding="utf-8", newline="\n")
(root / "README.md").write_text("# ledgerly\n\nSubscription billing. Run the tests with `python -m unittest discover -s tests -t .`\n", encoding="utf-8", newline="\n")
subprocess.run(["git", "init", "-q"], cwd=root, check=True)
subprocess.run(["git", "add", "."], cwd=root, check=True)
subprocess.run(["git", "-c", "user.name=jev-ab", "-c", "user.email=jev-ab@example.invalid", "commit", "-qm", "ledgerly"], cwd=root, check=True)
total = sum(p.stat().st_size for p in root.rglob("*.py"))
print(f"{root}: {len(list(root.rglob('*.py')))} python files, {total} bytes")
