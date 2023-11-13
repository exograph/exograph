import Stripe from "npm:stripe";

// public test key from https://stripe.com/docs/stripe-cli
const stripe = new Stripe('sk_test_4eC39HqLyjWDarjtT1zdp7dc');

export async function create_customer(email: string): Promise<string> {
	const customer = await stripe.customers.create({
		email: email,
	});

	return customer.id.substring(0, 4);
}
