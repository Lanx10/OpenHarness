import {useState} from 'react';
import {CheckIcon, ChevronDownIcon, CommandLineIcon, DocumentTextIcon, ExclamationCircleIcon, MagnifyingGlassIcon, PencilSquareIcon, ShieldExclamationIcon, SparklesIcon} from '@heroicons/react/20/solid';
import type {StepEventLike} from './StepEvent';

export function MessageCrumbs({steps, thinking}: {steps?: StepEventLike[]; thinking?: boolean}) {
  const [expanded, setExpanded] = useState<string | null>(null);
  const items = steps ?? [];
  const icon = (kind: string) => kind === 'read' ? DocumentTextIcon : kind === 'grep' ? MagnifyingGlassIcon : kind === 'edit' ? PencilSquareIcon : kind === 'skill' ? SparklesIcon : CommandLineIcon;
  if (!items.length && !thinking) return null;
  return <section className="message-crumbs" aria-label="Assistant activity">
    {thinking && <div className="message-crumb thinking"><span className="crumb-spinner" aria-hidden="true"/>Working</div>}
    {items.map(step => {
      const Icon = icon(step.kind);
      const isOpen = expanded === step.id;
      return <div className={`message-crumb-wrap status-${step.status}`} key={step.id}>
        <button type="button" className="message-crumb" aria-expanded={isOpen} onClick={() => setExpanded(isOpen ? null : step.id)}>
          {step.status === 'done' ? <CheckIcon width={13} height={13} aria-hidden="true"/> : step.status === 'error' ? <ExclamationCircleIcon width={13} height={13} aria-hidden="true"/> : step.status === 'approval' ? <ShieldExclamationIcon width={13} height={13} aria-hidden="true"/> : <Icon width={13} height={13} aria-hidden="true"/>}
          <span>{step.label}</span><span className="crumb-status">{step.status}</span><ChevronDownIcon width={12} height={12} aria-hidden="true"/>
        </button>
        {isOpen && step.detail && <div className="message-crumb-detail">{step.detail}</div>}
      </div>;
    })}
  </section>;
}
