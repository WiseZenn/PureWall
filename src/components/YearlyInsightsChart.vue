<script setup lang="ts">
import { computed } from "vue";
import type { MonthlyPlayStats } from "../stores/wallpapers";

const props = defineProps<{
  months: MonthlyPlayStats[];
  activeMonth?: number;
}>();

const labels = ["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"];
const maxPlays = computed(() => Math.max(1, ...props.months.map((month) => month.plays)));
const currentMonth = computed(() => props.activeMonth || new Date().getMonth() + 1);
</script>

<template>
  <div class="yearly-chart" aria-label="Yearly insights chart">
    <div class="yearly-chart__bars">
      <div
        v-for="month in months"
        :key="month.month"
        class="yearly-chart__bar-wrap"
        :title="`${labels[month.month - 1]}: ${month.plays} plays`"
      >
        <span
          class="yearly-chart__bar"
          :class="{ active: month.month === currentMonth }"
          :style="{ height: `${Math.max(10, (month.plays / maxPlays) * 100)}%` }"
        ></span>
      </div>
    </div>
    <div class="yearly-chart__labels">
      <span
        v-for="month in months"
        :key="month.month"
        :class="{ active: month.month === currentMonth }"
      >
        {{ labels[month.month - 1] }}
      </span>
    </div>
  </div>
</template>
