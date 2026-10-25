import time


def grow(n):
    chunks = []
    count = 0
    while count < n:
        chunks.append(bytes(range(256)) * (16 * 1024))
        time.sleep(0.01)
        count += 1
    return len(chunks)
