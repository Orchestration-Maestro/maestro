# Optimization patterns

## Backend and data

| Observation | Investigation and possible fix |
| --- | --- |
| One relation query per row | Observe the N+1 trace; fetch related data in a join or batch |
| Unbounded list | Use a limit and stable order; cursor pagination avoids large-offset work |
| Unexpected sequential scan | Read the actual query plan before adding an index |
| Row estimates far from actual | Refresh statistics before guessing at indexes |
| Sort above scan | Check whether an index matches equality, range and ordering |
| All endpoints waiting on connections | Find held connections and pool wait time, not just query time |
| CPU spikes | Profile heavy synchronous work, repeated computation and regex backtracking |
| Memory growth | Compare heap snapshots for retained objects, listeners and unbounded caches |

For indexes, measure the plan before and after. A dominant low-selectivity value may not benefit from a plain
index; a partial index may serve the rare value. Leading wildcards need an appropriate text-search strategy;
functions on a column may need an expression index. Include index write cost. Revert an index that does not
improve the measured plan and result.

Keep one connection pool per process. Across instances, total configured connections must fit the database's
capacity. Increasing the pool can relocate the queue into the database. For unbounded autoscaling, investigate
connection multiplexing rather than raising every instance's maximum.

## Caching

Cache expensive results read much more often than they change. Choose the layer deliberately: process,
shared store or edge. Include every response-changing input in the key, including tenant, viewer permissions,
locale and flags. Omission can leak another user's data.

State the acceptable staleness window and choose TTL, events/tags or versioned keys. Invalidate deliberately
on writes. Choose cache-aside (caller loads a miss), read-through (cache loads it), write-through (synchronous
cache/origin writes) or write-behind to fit consistency needs. Deferred writes need durability and failure
handling. Coalesce concurrent misses or serve stale while one owner refreshes to avoid stampedes.
Negative caching uses an explicit not-found sentinel and a shorter TTL than positive results, so newly
created records appear promptly. Never turn an origin error into cached absence. Set eviction and memory
bounds as well as a lifetime. Do not cache
balances, current permissions or checkout inventory when correctness needs fresh values. Measure hit rate,
miss latency and memory; caching a cheap result can add a network hop and a stale-data defect.

## Browser delivery and interaction

- Declare image width/height to reserve space. Supply modern formats and responsive sizes. Prioritize the
  LCP image; lazy-load and asynchronously decode below-fold images, not the initial critical image.
- Profile rerenders. Hoist stable values and memoize only demonstrably expensive work. Memoization itself
  consumes memory and comparison time; it is not a blanket remedy.
- Analyze bundle composition before changing import style. Split by route and defer heavy rarely used
  features. Verify tree-shaking, compression and long-lived fingerprinted static caching.
- Remove unused blocking CSS/scripts. Delay nonessential work; investigate DNS and TLS before adding
  preconnects to known origins. Measure HTTP/2, keep-alive and edge delivery in the actual deployment.
- Batch DOM reads and writes to avoid forced reflow. Use compositor-friendly animation and virtualize large
  lists when the profile shows rendering cost. Debounce/throttle only where the interaction permits it.
- Inspect API waterfalls, duplicate fetches, payload size and compression. Preserve cancellation, errors
  and required work while reducing redundant requests.
- Subset fonts, limit families/weights, preload only critical fonts and choose a display policy that avoids
  invisible text and excessive layout shifts.

## Validation checklist

Compare baseline and result under the same cache/network/load conditions. Record repeated measurements
and variance. Confirm correctness, query counts and plans, connection headroom, cache isolation/invalidation,
bundle size, image dimensions, layout shifts and interaction latency where applicable. Check for cleanup of
listeners, retained objects and workers on repeated navigation. A synthetic pass needs field confirmation;
a desktop-only result does not establish a mobile experience. Retain a regression budget for the metric
that justified the change, and log failed experiments as well as successful ones.
