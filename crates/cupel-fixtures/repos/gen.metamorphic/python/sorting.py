def sort_list(xs):
    """Returns the elements of xs in ascending order."""
    result = list(xs)
    for i in range(len(result) - 1):
        if result[i] > result[i + 1]:
            result[i], result[i + 1] = result[i + 1], result[i]
    return result
