# ABOUTME: The depended-on root's code: `fee`, whose own test never charges a large order, and `waived`,
# ABOUTME: which only the dependent root's tests call.
def fee(total):
    if total >= 100:
        return 0
    return 5


def waived(code):
    return code == "FREE"
