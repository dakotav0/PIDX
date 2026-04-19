<script lang="ts">
	import { page } from '$app/state';

	const userId = $derived(page.params.user);
</script>

<p>Profile: {userId}</p>
<!-- Phase 3+: Inspector, Debugger, Gardener views -->
