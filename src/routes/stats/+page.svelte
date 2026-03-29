<script>
  import { onMount } from 'svelte';
  import { getAllDailyStats, getWritingSettings, updateWritingSettings, getManuscriptWordHistory, getHourlyBreakdown } from '$lib/db.js';

  let stats = $state([]);
  let settings = $state({ daily_goal: 1000, session_gap_minutes: 30 });
  let wordHistory = $state([]);
  let loading = $state(true);

  let calendarCanvas;
  let growthCanvas;
  let dailyCanvas;
  let calendarCells = [];
  let tooltip = $state(null);
  let calendarYear = $state(new Date().getFullYear());
  let selectedDay = $state(null); // date string
  let hourlyData = $state([]);
  let hourlyLoading = $state(false);
  let hourlyCanvas;

  // Local date string helper — avoids UTC timezone issues
  function toLocalDateStr(d) {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${y}-${m}-${day}`;
  }

  onMount(async () => {
    try {
      stats = await getAllDailyStats();
      settings = await getWritingSettings();
      wordHistory = await getManuscriptWordHistory();
    } catch (e) { console.warn('Stats load failed:', e); }
    loading = false;
  });

  $effect(() => { if (!loading && calendarCanvas && stats.length >= 0) { const _y = calendarYear; drawCalendar(); } });
  $effect(() => { if (!loading && growthCanvas && wordHistory.length > 0) drawGrowthChart(); });
  $effect(() => { if (!loading && dailyCanvas && stats.length > 0) drawDailyChart(); });

  async function selectDay(dateStr) {
    if (selectedDay === dateStr) { selectedDay = null; return; }
    selectedDay = dateStr;
    hourlyLoading = true;
    try {
      console.log('[stats] fetching hourly for:', dateStr);
      hourlyData = await getHourlyBreakdown(dateStr);
      console.log('[stats] hourly data:', hourlyData);
    } catch (e) { console.error('[stats] hourly error:', e); hourlyData = []; }
    hourlyLoading = false;
  }

  $effect(() => {
    if (selectedDay && hourlyData.length > 0) {
      // Wait a tick for the canvas to mount
      setTimeout(() => { if (hourlyCanvas) drawHourlyChart(); }, 50);
    }
  });

  function drawHourlyChart() {
    const canvas = hourlyCanvas;
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const containerW = canvas.parentElement?.clientWidth || 500;
    const barW = Math.max(16, (containerW - 60) / 24 - 2);
    const logicalW = containerW;
    const logicalH = 160;
    const padL = 45, padB = 25, padT = 15;
    const chartH = logicalH - padT - padB;

    canvas.width = logicalW * dpr;
    canvas.height = logicalH * dpr;
    canvas.style.width = logicalW + 'px';
    canvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    // Build full 24-hour array
    const hours = Array.from({ length: 24 }, (_, i) => {
      const h = hourlyData.find(d => d.hour === i);
      return h ? h.net_words : 0;
    });
    const maxVal = Math.max(...hours.map(Math.abs), 1);

    // Zero line
    const zeroY = padT + chartH / 2;
    ctx.strokeStyle = '#e5e1da';
    ctx.lineWidth = 1;
    ctx.beginPath(); ctx.moveTo(padL, zeroY); ctx.lineTo(logicalW - 10, zeroY); ctx.stroke();

    for (let i = 0; i < 24; i++) {
      const x = padL + i * (barW + 2);
      const val = hours[i];
      if (val === 0) continue;

      const h = Math.abs(val) / maxVal * (chartH / 2);
      if (val > 0) {
        ctx.fillStyle = '#2d6a5e';
        ctx.fillRect(x, zeroY - h, barW, h);
        ctx.fillStyle = '#6b6560';
        ctx.font = '9px Source Sans 3, system-ui';
        ctx.textAlign = 'center';
        ctx.fillText('+' + val, x + barW / 2, zeroY - h - 3);
      } else {
        ctx.fillStyle = '#b85450';
        ctx.fillRect(x, zeroY, barW, h);
        ctx.fillStyle = '#6b6560';
        ctx.font = '9px Source Sans 3, system-ui';
        ctx.textAlign = 'center';
        ctx.fillText(val.toString(), x + barW / 2, zeroY + h + 10);
      }

      // Hour label
      ctx.fillStyle = '#9e9891';
      ctx.font = '9px Source Sans 3, system-ui';
      ctx.textAlign = 'center';
      const ampm = i === 0 ? '12a' : i < 12 ? `${i}a` : i === 12 ? '12p' : `${i - 12}p`;
      ctx.fillText(ampm, x + barW / 2, logicalH - 5);
    }
  }

  async function saveSettings() {
    await updateWritingSettings(settings.daily_goal, settings.session_gap_minutes);
  }

  // --- Computed stats ---
  let today = $derived(() => {
    const d = toLocalDateStr(new Date());
    return stats.find(s => s.date === d) || { words_added: 0, words_deleted: 0, net_words: 0, active_minutes: 0 };
  });

  let streak = $derived(() => {
    let count = 0;
    const sorted = [...stats].sort((a, b) => b.date.localeCompare(a.date));
    const todayStr = toLocalDateStr(new Date());
    let checkDate = new Date();

    for (let i = 0; i < 365; i++) {
      const dateStr = toLocalDateStr(checkDate);
      const day = sorted.find(s => s.date === dateStr);
      if (day && day.net_words > 0) {
        count++;
      } else if (i > 0) { // allow today to be 0
        break;
      }
      checkDate.setDate(checkDate.getDate() - 1);
    }
    return count;
  });

  let bestDay = $derived(() => {
    if (stats.length === 0) return { date: '', net_words: 0 };
    return stats.reduce((best, s) => s.net_words > best.net_words ? s : best, stats[0]);
  });

  let totalWritten = $derived(() => stats.reduce((s, d) => s + d.words_added, 0));
  let totalDays = $derived(() => stats.filter(s => s.net_words > 0).length);
  let avgPerDay = $derived(() => totalDays() > 0 ? Math.round(totalWritten() / totalDays()) : 0);

  // --- Calendar heatmap ---
  function drawCalendar() {
    const canvas = calendarCanvas;
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;

    const cellSize = 18;
    const cellGap = 3;
    const labelW = 40;
    const headerH = 25;

    // Jan 1 to Dec 31 of selected year
    const yearStart = new Date(calendarYear, 0, 1);
    const yearEnd = new Date(calendarYear, 11, 31);
    const today = new Date();

    // Start from the Sunday of the week containing Jan 1
    const startDate = new Date(yearStart);
    startDate.setDate(startDate.getDate() - startDate.getDay());

    // End at the Saturday of the week containing Dec 31
    const endDate = new Date(yearEnd);
    endDate.setDate(endDate.getDate() + (6 - endDate.getDay()));

    const totalDaysSpan = Math.round((endDate - startDate) / 86400000) + 1;
    const weeksToShow = Math.ceil(totalDaysSpan / 7);

    const logicalW = labelW + weeksToShow * (cellSize + cellGap) + 20;
    const logicalH = headerH + 7 * (cellSize + cellGap) + 10;

    canvas.width = logicalW * dpr;
    canvas.height = logicalH * dpr;
    canvas.style.width = logicalW + 'px';
    canvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    // Build date → words map
    const dateMap = {};
    for (const s of stats) {
      dateMap[s.date] = s.net_words;
    }

    // Day labels
    const dayLabels = ['Sun', 'Mon', '', 'Wed', '', 'Fri', ''];
    ctx.font = '11px Source Sans 3, system-ui';
    ctx.fillStyle = '#9e9891';
    ctx.textAlign = 'right';
    for (let i = 0; i < 7; i++) {
      ctx.fillText(dayLabels[i], labelW - 6, headerH + i * (cellSize + cellGap) + cellSize - 3);
    }

    let monthLabels = [];
    let lastMonth = -1;
    calendarCells = [];

    for (let week = 0; week < weeksToShow; week++) {
      for (let day = 0; day < 7; day++) {
        const d = new Date(startDate);
        d.setDate(d.getDate() + week * 7 + day);

        // Only draw cells for the selected year
        if (d.getFullYear() !== calendarYear) continue;
        // Don't draw future dates
        if (d > today) continue;

        const dateStr = toLocalDateStr(d);
        const words = dateMap[dateStr] || 0;
        const x = labelW + week * (cellSize + cellGap);
        const y = headerH + day * (cellSize + cellGap);

        calendarCells.push({ x, y, size: cellSize, date: dateStr, words });

        // Month label
        if (d.getMonth() !== lastMonth && day === 0) {
          lastMonth = d.getMonth();
          monthLabels.push({ x, month: d.toLocaleString('default', { month: 'short' }) });
        }

        if (words <= 0) {
          ctx.fillStyle = '#eeeae4';
        } else {
          const intensity = Math.min(1, words / (settings.daily_goal || 1000));
          const r = Math.round(45 + (1 - intensity) * 190);
          const g = Math.round(106 + (1 - intensity) * 130);
          const b = Math.round(94 + (1 - intensity) * 140);
          ctx.fillStyle = `rgb(${r}, ${g}, ${b})`;
        }

        ctx.beginPath();
        ctx.roundRect(x, y, cellSize, cellSize, 3);
        ctx.fill();
      }
    }

    // Month labels
    ctx.font = '11px Source Sans 3, system-ui';
    ctx.fillStyle = '#6b6560';
    ctx.textAlign = 'left';
    for (const ml of monthLabels) {
      ctx.fillText(ml.month, ml.x, headerH - 8);
    }
  }

  // --- Manuscript growth chart ---
  function drawGrowthChart() {
    const canvas = growthCanvas;
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const data = wordHistory;
    if (data.length < 2) return;

    const containerW = canvas.parentElement?.clientWidth || 600;
    const logicalW = containerW;
    const logicalH = 200;
    const padL = 60, padR = 20, padT = 20, padB = 30;

    canvas.width = logicalW * dpr;
    canvas.height = logicalH * dpr;
    canvas.style.width = logicalW + 'px';
    canvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    const maxVal = Math.max(...data.map(d => d[1]), 1);
    const chartW = logicalW - padL - padR;
    const chartH = logicalH - padT - padB;

    // Draw line
    ctx.beginPath();
    ctx.strokeStyle = '#2d6a5e';
    ctx.lineWidth = 2;
    for (let i = 0; i < data.length; i++) {
      const x = padL + (i / (data.length - 1)) * chartW;
      const y = padT + chartH - (data[i][1] / maxVal) * chartH;
      if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    }
    ctx.stroke();

    // Fill under
    ctx.lineTo(padL + chartW, padT + chartH);
    ctx.lineTo(padL, padT + chartH);
    ctx.closePath();
    ctx.fillStyle = 'rgba(45, 106, 94, 0.1)';
    ctx.fill();

    // Y axis labels
    ctx.font = '10px Source Sans 3, system-ui';
    ctx.fillStyle = '#9e9891';
    ctx.textAlign = 'right';
    for (let i = 0; i <= 4; i++) {
      const val = Math.round(maxVal * i / 4);
      const y = padT + chartH - (i / 4) * chartH;
      ctx.fillText(val.toLocaleString(), padL - 8, y + 4);
      ctx.strokeStyle = '#eeeae4';
      ctx.lineWidth = 0.5;
      ctx.beginPath(); ctx.moveTo(padL, y); ctx.lineTo(padL + chartW, y); ctx.stroke();
    }

    // X axis labels
    ctx.textAlign = 'center';
    const step = Math.max(1, Math.floor(data.length / 6));
    for (let i = 0; i < data.length; i += step) {
      const x = padL + (i / (data.length - 1)) * chartW;
      ctx.fillText(data[i][0].slice(5), x, logicalH - 5); // MM-DD
    }
  }

  // --- Daily words bar chart (last 30 days) ---
  function drawDailyChart() {
    const canvas = dailyCanvas;
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;

    // Last 30 days
    const days = [];
    const today = new Date();
    for (let i = 29; i >= 0; i--) {
      const d = new Date(today);
      d.setDate(d.getDate() - i);
      const dateStr = toLocalDateStr(d);
      const stat = stats.find(s => s.date === dateStr);
      days.push({ date: dateStr, day: d.getDate(), net: stat ? stat.net_words : 0 });
    }

    const containerW = canvas.parentElement?.clientWidth || 600;
    const barW = Math.max(12, (containerW - 60) / 30 - 2);
    const logicalW = containerW;
    const logicalH = 180;
    const padL = 50, padB = 25, padT = 15;
    const chartH = logicalH - padT - padB;

    canvas.width = logicalW * dpr;
    canvas.height = logicalH * dpr;
    canvas.style.width = logicalW + 'px';
    canvas.style.height = logicalH + 'px';
    ctx.scale(dpr, dpr);
    ctx.fillStyle = '#faf8f5';
    ctx.fillRect(0, 0, logicalW, logicalH);

    const maxVal = Math.max(...days.map(d => d.net), settings.daily_goal || 1000, 1);

    // Goal line
    const goalY = padT + chartH - ((settings.daily_goal || 1000) / maxVal) * chartH;
    ctx.strokeStyle = '#d97706';
    ctx.lineWidth = 1;
    ctx.setLineDash([4, 4]);
    ctx.beginPath(); ctx.moveTo(padL, goalY); ctx.lineTo(logicalW - 10, goalY); ctx.stroke();
    ctx.setLineDash([]);
    ctx.font = '9px Source Sans 3, system-ui';
    ctx.fillStyle = '#d97706';
    ctx.textAlign = 'left';
    ctx.fillText('Goal', logicalW - 30, goalY - 3);

    // Bars
    for (let i = 0; i < days.length; i++) {
      const x = padL + i * (barW + 2);
      const h = Math.max(0, (days[i].net / maxVal) * chartH);
      const y = padT + chartH - h;
      const metGoal = days[i].net >= (settings.daily_goal || 1000);
      ctx.fillStyle = metGoal ? '#2d6a5e' : '#9ecac2';
      ctx.fillRect(x, y, barW, h);

      // Day label
      if (i % 3 === 0 || i === days.length - 1) {
        ctx.font = '9px Source Sans 3, system-ui';
        ctx.fillStyle = '#9e9891';
        ctx.textAlign = 'center';
        ctx.fillText(days[i].day.toString(), x + barW / 2, logicalH - 5);
      }
    }
  }
</script>

<div class="stats-page">
  <div class="stats-toolbar">
    <h1 class="stats-title">Writing Stats</h1>
    <div class="stats-settings">
      <label class="setting">
        Daily goal:
        <input type="number" bind:value={settings.daily_goal} min="50" max="10000" step="50" class="setting-input" onblur={saveSettings} />
        <span class="setting-unit">words</span>
      </label>
      <label class="setting">
        Session gap:
        <input type="number" bind:value={settings.session_gap_minutes} min="5" max="120" class="setting-input" onblur={saveSettings} />
        <span class="setting-unit">min</span>
      </label>
    </div>
  </div>

  {#if loading}
    <div class="stats-loading">Loading stats...</div>
  {:else}
    <div class="stats-content">
      <!-- Today's stats -->
      <div class="today-grid">
        <div class="today-card accent">
          <span class="today-val">{today().net_words.toLocaleString()}</span>
          <span class="today-label">Words Today</span>
          {#if settings.daily_goal > 0}
            <div class="goal-bar">
              <div class="goal-fill" style="width: {Math.min(100, (today().net_words / settings.daily_goal) * 100)}%"></div>
            </div>
            <span class="goal-text">{Math.round((today().net_words / settings.daily_goal) * 100)}% of goal</span>
          {/if}
        </div>
        <div class="today-card">
          <span class="today-val">{streak()}</span>
          <span class="today-label">Day Streak</span>
        </div>
        <div class="today-card">
          <span class="today-val">{today().active_minutes}</span>
          <span class="today-label">Minutes Active</span>
        </div>
        <div class="today-card">
          <span class="today-val">{avgPerDay().toLocaleString()}</span>
          <span class="today-label">Avg Words/Day</span>
        </div>
        <div class="today-card">
          <span class="today-val">{bestDay().net_words.toLocaleString()}</span>
          <span class="today-label">Best Day</span>
        </div>
        <div class="today-card">
          <span class="today-val">{totalDays()}</span>
          <span class="today-label">Days Writing</span>
        </div>
      </div>

      <!-- Calendar heatmap -->
      <div class="chart-section">
        <div class="chart-title-row">
          <h2 class="chart-title">Writing Calendar</h2>
          <div class="year-picker">
            <button class="year-btn" onclick={() => calendarYear--}><i class="bi bi-chevron-left"></i></button>
            <span class="year-label">{calendarYear}</span>
            <button class="year-btn" onclick={() => { if (calendarYear < new Date().getFullYear()) calendarYear++; }} disabled={calendarYear >= new Date().getFullYear()}>
              <i class="bi bi-chevron-right"></i>
            </button>
          </div>
        </div>
        <p class="chart-hint">Darker = more words written. Hover for details.</p>
        <div class="chart-wrap" style="position: relative; overflow: visible;">
          <canvas
            bind:this={calendarCanvas}
            onmousemove={e => {
              const rect = calendarCanvas.getBoundingClientRect();
              const mx = e.clientX - rect.left;
              const my = e.clientY - rect.top;
              const cell = calendarCells.find(c => mx >= c.x && mx < c.x + c.size && my >= c.y && my < c.y + c.size);
              if (cell) {
                const d = new Date(cell.date + 'T12:00:00');
                const label = d.toLocaleDateString('default', { weekday: 'short', month: 'short', day: 'numeric' });
                tooltip = { text: `${label}: ${cell.words.toLocaleString()} words`, x: cell.x + cell.size / 2, y: cell.y - 8 };
              } else {
                tooltip = null;
              }
            }}
            onmouseleave={() => tooltip = null}
            onclick={e => {
              const rect = calendarCanvas.getBoundingClientRect();
              const mx = e.clientX - rect.left;
              const my = e.clientY - rect.top;
              const cell = calendarCells.find(c => mx >= c.x && mx < c.x + c.size && my >= c.y && my < c.y + c.size);
              if (cell) selectDay(cell.date);
            }}
          ></canvas>
          {#if tooltip}
            <div class="cal-tooltip" style="left: {tooltip.x}px; top: {tooltip.y}px;">
              {tooltip.text}
            </div>
          {/if}
        </div>
      </div>

      <!-- Hourly breakdown for selected day -->
      {#if selectedDay}
        <div class="chart-section hourly-section">
          <div class="chart-title-row">
            <h2 class="chart-title">
              {new Date(selectedDay + 'T12:00:00').toLocaleDateString('default', { weekday: 'long', month: 'long', day: 'numeric', year: 'numeric' })}
            </h2>
            <button class="year-btn" onclick={() => selectedDay = null}><i class="bi bi-x-lg"></i></button>
          </div>
          {#if hourlyLoading}
            <p class="chart-hint">Loading...</p>
          {:else if hourlyData.length === 0}
            <p class="chart-hint">No writing activity recorded this day.</p>
          {:else}
            {@const dayNet = hourlyData.reduce((s, h) => s + h.net_words, 0)}
            {@const dayAdded = hourlyData.reduce((s, h) => s + h.words_added, 0)}
            {@const dayDeleted = hourlyData.reduce((s, h) => s + h.words_deleted, 0)}
            {@const activeHours = hourlyData.filter(h => h.events > 0).length}
            <div class="hourly-summary">
              <span class="hourly-stat"><strong>{dayNet.toLocaleString()}</strong> net words</span>
              <span class="hourly-sep">&middot;</span>
              <span class="hourly-stat"><strong>+{dayAdded.toLocaleString()}</strong> added</span>
              <span class="hourly-sep">&middot;</span>
              <span class="hourly-stat"><strong>-{dayDeleted.toLocaleString()}</strong> deleted</span>
              <span class="hourly-sep">&middot;</span>
              <span class="hourly-stat"><strong>{activeHours}</strong> active hours</span>
            </div>
            <p class="chart-hint">Green = words added, red = words deleted. Each bar is one hour.</p>
            <div class="chart-wrap"><canvas bind:this={hourlyCanvas}></canvas></div>
          {/if}
        </div>
      {/if}

      <!-- Last 30 days -->
      <div class="chart-section">
        <h2 class="chart-title">Last 30 Days</h2>
        <p class="chart-hint">Dashed line = daily goal. Dark bars = goal met.</p>
        <div class="chart-wrap"><canvas bind:this={dailyCanvas}></canvas></div>
      </div>

      <!-- Manuscript growth -->
      <div class="chart-section">
        <h2 class="chart-title">Manuscript Growth</h2>
        <p class="chart-hint">Total word count over time. Watch your book grow.</p>
        <div class="chart-wrap"><canvas bind:this={growthCanvas}></canvas></div>
      </div>
    </div>
  {/if}
</div>

<style>
  :global(html), :global(body) { overflow: auto !important; height: auto !important; }

  .stats-page {
    font-family: 'Source Sans 3', system-ui, sans-serif;
    background: #faf8f5; min-height: 100vh;
  }
  .stats-toolbar {
    display: flex; align-items: center; justify-content: space-between;
    padding: 1rem 1.5rem; border-bottom: 1px solid #e5e1da; background: white;
  }
  .stats-title {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1.2rem; font-weight: 400; margin: 0; color: #2d2a26;
  }
  .stats-settings { display: flex; gap: 1rem; align-items: center; }
  .setting { display: flex; align-items: center; gap: 0.3rem; font-size: 0.8rem; color: #6b6560; }
  .setting-input {
    width: 60px; padding: 0.25rem 0.3rem; border: 1px solid #e5e1da;
    border-radius: 4px; font-size: 0.8rem; text-align: center;
  }
  .setting-unit { font-size: 0.7rem; color: #9e9891; }

  .stats-loading {
    display: flex; align-items: center; justify-content: center;
    height: 50vh; color: #9e9891; font-style: italic;
  }
  .stats-content { padding: 1.5rem; }

  .today-grid {
    display: grid; grid-template-columns: repeat(3, 1fr); gap: 1px;
    background: #e5e1da; border-radius: 8px; overflow: hidden; margin-bottom: 1.5rem;
  }
  .today-card {
    display: flex; flex-direction: column; align-items: center;
    padding: 1rem; background: white;
  }
  .today-card.accent { background: #f0faf7; }
  .today-val {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1.8rem; font-weight: 600; color: #2d2a26;
  }
  .today-label {
    font-size: 0.7rem; color: #9e9891; text-transform: uppercase;
    letter-spacing: 0.06em; margin-top: 0.15rem;
  }
  .goal-bar {
    width: 80%; height: 6px; background: #e5e1da; border-radius: 3px;
    margin-top: 0.5rem; overflow: hidden;
  }
  .goal-fill { height: 100%; background: #2d6a5e; border-radius: 3px; transition: width 0.5s; }
  .goal-text { font-size: 0.65rem; color: #2d6a5e; margin-top: 0.2rem; }

  .chart-section { margin-bottom: 1.5rem; }
  .chart-title {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1rem; font-weight: 400; margin: 0 0 0.2rem; color: #2d2a26;
  }
  .chart-hint { font-size: 0.75rem; color: #9e9891; margin: 0 0 0.5rem; font-style: italic; }
  .chart-wrap {
    overflow-x: auto; border: 1px solid #e5e1da; border-radius: 6px;
    background: #faf8f5; padding: 0.75rem;
  }
  .chart-wrap canvas { display: block; }

  .hourly-section {
    background: white; border: 1px solid #e5e1da; border-radius: 8px;
    padding: 1rem; margin-bottom: 1.5rem;
  }
  .hourly-summary {
    display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;
    font-size: 0.85rem; color: #6b6560; margin-bottom: 0.5rem;
  }
  .hourly-stat strong { color: #2d2a26; }
  .hourly-sep { color: #d0ccc6; }

  .cal-tooltip {
    position: absolute; transform: translate(-50%, -100%);
    background: #2d2a26; color: white; font-size: 0.8rem;
    padding: 0.35rem 0.7rem; border-radius: 4px;
    white-space: nowrap; pointer-events: none;
    box-shadow: 0 2px 8px rgba(0,0,0,0.2);
    z-index: 100;
  }

  .chart-title-row {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: 0.2rem;
  }
  .year-picker {
    display: flex; align-items: center; gap: 0.4rem;
  }
  .year-btn {
    background: none; border: 1px solid #e5e1da; border-radius: 4px;
    cursor: pointer; padding: 0.2rem 0.4rem; color: #6b6560;
    display: flex; align-items: center; transition: all 150ms;
  }
  .year-btn:hover:not(:disabled) { border-color: #2d6a5e; color: #2d6a5e; }
  .year-btn:disabled { opacity: 0.3; cursor: default; }
  .year-label {
    font-family: 'Libre Baskerville', Georgia, serif;
    font-size: 1rem; font-weight: 600; color: #2d2a26;
    min-width: 50px; text-align: center;
  }
</style>
