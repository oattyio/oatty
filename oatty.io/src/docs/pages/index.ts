import {accessReviewCollectionPlaybookPage} from './access-review-collection-playbook-page';
import {cliCommandsPage} from './cli-commands-page';
import {credentialRotationReadinessPlaybookPage} from './credential-rotation-readiness-playbook-page';
import type {DocsPage} from '../types';
import {gettingOrientedPage} from './getting-oriented-page';
import {howOattyExecutesSafelyPage} from './how-oatty-executes-safely-page';
import {libraryAndCatalogsPage} from './library-and-catalogs-page';
import {mcpHttpServerPage} from './mcp-http-server-page';
import {pluginsPage} from './plugins-page';
import {quickStartPage} from './quick-start-page';
import {searchAndRunCommandsPage} from './search-and-run-commands-page';
import {sentryBootstrapPage} from './sentry-bootstrap-page';
import {sentryDatadogPagerDutyPlaybookPage} from './sentry-datadog-pagerduty-playbook-page';
import {tuiInteractionsPage} from './tui-interactions-page';
import {vercelToRenderMigrationPlaybookPage} from './vercel-to-render-migration-playbook-page';
import {workflowsBasicsPage} from './workflows-basics-page';

/**
 * Ordered docs pages used for routing and prev/next navigation.
 */
export const docsPages: DocsPage[] = [
    quickStartPage,
    sentryBootstrapPage,
    sentryDatadogPagerDutyPlaybookPage,
    vercelToRenderMigrationPlaybookPage,
    accessReviewCollectionPlaybookPage,
    credentialRotationReadinessPlaybookPage,
    howOattyExecutesSafelyPage,
    gettingOrientedPage,
    libraryAndCatalogsPage,
    searchAndRunCommandsPage,
    workflowsBasicsPage,
    pluginsPage,
    mcpHttpServerPage,
    cliCommandsPage,
    tuiInteractionsPage,
];
