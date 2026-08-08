<script setup lang="ts">
import { computed } from "vue";
import { useWallpaperStore } from "../stores/wallpapers";

const store = useWallpaperStore();

const maxMonthlyPlays = computed(() =>
  Math.max(1, ...store.yearlyStats.monthly.map((item) => item.plays)),
);

function fileName(path: string) {
  return path.split(/[\\/]/).pop() || path;
}
</script>

<template>
  <section class="insights-panel">
    <div class="flex flex-wrap items-start gap-4">
      <div class="min-w-[130px]">
        <div class="text-xs" style="color: var(--text-secondary)">Year</div>
        <div class="text-2xl font-semibold leading-tight" style="color: var(--text-primary)">
          {{ store.yearlyStats.year }}
        </div>
      </div>

      <div class="grid grid-cols-3 gap-3 flex-1 min-w-[320px]">
        <div class="insight-metric">
          <div class="text-lg font-semibold">{{ store.yearlyStats.total_plays }}</div>
          <div>Plays</div>
        </div>
        <div class="insight-metric">
          <div class="text-lg font-semibold">{{ store.yearlyStats.unique_wallpapers }}</div>
          <div>Seen</div>
        </div>
        <div class="insight-metric">
          <div class="text-lg font-semibold">{{ store.yearlyStats.liked_plays }}</div>
          <div>Liked plays</div>
        </div>
      </div>
    </div>

    <div class="mt-4 grid grid-cols-1 xl:grid-cols-[1fr_260px] gap-4">
      <div class="monthly-bars" aria-label="Monthly plays">
        <div
          v-for="month in store.yearlyStats.monthly"
          :key="month.month"
          class="monthly-bar"
        >
          <div class="bar-track">
            <div
              class="bar-fill"
              :style="{ height: `${Math.max(8, (month.plays / maxMonthlyPlays) * 100)}%` }"
            ></div>
          </div>
          <div class="bar-label">{{ month.month }}</div>
        </div>
      </div>

      <div class="top-wallpapers">
        <div class="text-xs mb-2" style="color: var(--text-secondary)">Top wallpapers</div>
        <div v-if="store.yearlyStats.top_wallpapers.length === 0" class="top-empty">
          No plays tracked
        </div>
        <div
          v-for="item in store.yearlyStats.top_wallpapers"
          :key="item.path"
          class="top-row"
        >
          <span class="truncate">{{ fileName(item.path) }}</span>
          <span>{{ item.plays }}</span>
        </div>
      </div>
    </div>
  </section>
</template>
