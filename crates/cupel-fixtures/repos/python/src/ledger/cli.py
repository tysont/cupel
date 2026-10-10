import sys

from ledger import balance


def main(argv):
    return balance([int(arg) for arg in argv])


def run():
    print(main(sys.argv[1:]))


if __name__ == "__main__":
    run()
