"""Experiment inputs, separate from deployment policy and application semantics."""
SOURCE = r'''
#define _POSIX_C_SOURCE 199309L
#include <stdio.h>
#include <time.h>
#include <unistd.h>
#include <sys/stat.h>
int main(void) {
  long value, millis;
  if(scanf("%ld %ld", &value, &millis)!=2) return 10;
  struct stat st;
  if(!stat("/tmp/traojudge-sentinel", &st) || !stat("outputs/sentinel", &st)) return 11;
  FILE *a=fopen("/tmp/traojudge-sentinel","w"); if(!a) return 12; fputs("private",a); fclose(a);
  FILE *b=fopen("outputs/sentinel","w"); if(!b) return 13; fputs("private",b); fclose(b);
  if(fork()==0) { struct timespec t={30,0}; nanosleep(&t,0); _exit(0); }
  struct timespec wait={millis/1000,(millis%1000)*1000000}; nanosleep(&wait,0);
  printf("%ld\n",value); return 0;
}
'''


def body(contest, request, delays, parents=None, fail_case=None, **fault):
    return {"contest": contest, "request_id": request, "source": SOURCE,
        "tests": [{"input": f"{i} {ms}\n", "expected": f"{i if i != fail_case else -100}\n"}
                  for i, ms in enumerate(delays)],
        "test_parents": parents if parents is not None else [None] * len(delays),
        "limits": {"wall_seconds": 20, "cpu_seconds": 2, "memory_mb": 128}, **fault}


def forecast_tree(contest, request):
    # A long narrow gate chain gives a cold VM time to boot before the wide layer.
    # These are workload timings, not hard-coded autoscaling decisions.
    return body(contest, request, [18000] * 8 + [6000] * 5,
                parents=[None] + list(range(7)) + [7] * 5)


def pruned_tree(contest, request):
    return body(contest, request, [100, 100, 100, 100, 1000, 100],
                parents=[None, 0, 0, 1, None, 4], fail_case=0)
