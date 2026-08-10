import { GetServerSidePropsContext } from "next";

function SlugPage() {
  return <></>;
}

export async function getServerSideProps(context: GetServerSidePropsContext) {
  if (context.res) {
    context.res.writeHead(301, {
      Location: `/content/${context.query.slug}`
    });
    context.res.end();
  }
  return { props: {} };
}

export default SlugPage;
