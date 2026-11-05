import Tooltip from './Tooltip';

/** A small "i" that explains a setting on hover or focus. */
export default function InfoTip({ text }: { text: string }) {
  return (
    <Tooltip content={text}>
      <button type="button" className="ui-infotip" aria-label="More information">i</button>
    </Tooltip>
  );
}
