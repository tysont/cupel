#include <pthread.h>
#include <stdio.h>

int total;

void *count(void *unused) {
  for (int i = 0; i < 1000; i++) total++;
  return 0;
}

int main(void) {
  pthread_t first, second;
  pthread_create(&first, 0, count, 0);
  pthread_create(&second, 0, count, 0);
  pthread_join(first, 0);
  pthread_join(second, 0);
  printf("%d\n", total);
  return 0;
}
