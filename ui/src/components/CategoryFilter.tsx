export const categoryLabels = {
  all: '全部分区',
  battle: '战斗与技能',
  story: '剧情与叙事',
  system: '系统与界面',
} as const;

export type Category = keyof typeof categoryLabels;
const categories: readonly Category[] = ['all', 'battle', 'story', 'system'];

export function CategoryFilter({
  id,
  value,
  onChange,
}: {
  id: string;
  value: Category;
  onChange: (category: Category) => void;
}) {
  return (
    <div className="category-filter" role="group" aria-label="按分域进度分类">
      {categories.map((category) => (
        <button
          key={category}
          type="button"
          data-testid={`${id}-category-${category}`}
          aria-pressed={value === category}
          onClick={() => onChange(category)}
        >
          {categoryLabels[category]}
        </button>
      ))}
    </div>
  );
}
